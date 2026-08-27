#!/usr/bin/env bash
set -euo pipefail

readonly APP_USER=steam
readonly APP_UID=1000
readonly APP_GID=1000
readonly HOME_DIR=/mnt/data
readonly STATE_DIR=${HOME_DIR}/.config/steam-remote
readonly RUNTIME_DIR=/run/user/${APP_UID}
readonly CONTROL_DIR=/run/steam-remote
readonly DBUS_ADDRESS=unix:path=${RUNTIME_DIR}/bus
readonly GAMESCOPE_DISPLAY=gamescope-0
readonly GAMESCOPE_SOCKET=${RUNTIME_DIR}/${GAMESCOPE_DISPLAY}
readonly LIFECYCLE_FILE=${CONTROL_DIR}/lifecycle
readonly CONTENT_LOG=${HOME_DIR}/.steam/steam/logs/content_log.txt
readonly WIDTH=${STEAM_REMOTE_WIDTH:-3840}
readonly HEIGHT=${STEAM_REMOTE_HEIGHT:-2160}
readonly FPS=${STEAM_REMOTE_FPS:-60}
readonly PHYS_MM=${STEAM_REMOTE_PHYS_MM:-596x335}
# Withholding NV12 forces Steam onto a zero-copy BGRx DMA-BUF capture, which the
# GPU converts to NV12 in-place: ~3x less CPU than the NV12 shared-memory path
# (Steam always prefers NV12 when both are offered, so it must be withheld). The
# only observed failure is a transient VA surface allocation during a mid-session
# resolution change, which the pinned client resolution avoids; set to 0 to
# revert to the always-safe shared-memory path.
readonly ZERO_COPY=${STEAM_REMOTE_ZERO_COPY:-1}
readonly CURSOR_CANVAS=${STEAM_REMOTE_CURSOR_CANVAS:-96}
readonly CURSOR_GLYPH=${STEAM_REMOTE_CURSOR_GLYPH:-30}
readonly IDLE_SECONDS=300
readonly PARKED_FPS=1
readonly LIFECYCLE_RETRY_SECONDS=5
readonly LIFECYCLE_STALE_SECONDS=10
# Steam applies pending client updates only on launch. With no activity for
# AUTO_UPDATE_PARKED_SECONDS beyond parking, and at most once per
# AUTO_UPDATE_INTERVAL_SECONDS, the session restarts in place so a
# perpetually idle host still picks them up. STEAM_REMOTE_AUTO_UPDATE=0
# disables the restart.
readonly AUTO_UPDATE=${STEAM_REMOTE_AUTO_UPDATE:-1}
readonly AUTO_UPDATE_MARKER=${CONTROL_DIR}/restart-request
readonly AUTO_UPDATE_STAMP=${STATE_DIR}/last-auto-update
readonly AUTO_UPDATE_PARKED_SECONDS=3600
readonly AUTO_UPDATE_INTERVAL_SECONDS=86400
readonly SESSION_RESTART_STATUS=75
# While parked, everything not needed to accept a new Remote Play session is
# suspended with SIGSTOP so the GPU and CPU go fully idle. The main steam
# process stays running for discovery and connection accept; the kernel
# completes a client's TCP handshake on its own, the supervisor notices the
# established connection within a second and resumes everything first.
# Xwayland must NOT be suspended: steam is an X11 client, and a frozen X
# server stalls the very thread that answers discovery probes, making the
# host invisible to clients. With the web helper suspended nothing draws, so
# a running Xwayland costs nothing and keeps steam responsive.
readonly -a DEEP_PARK_NAMES=(gamescope gamescope-wl steamwebhelper)

declare -a children=()
declare -a auxiliaries=()

user_command() {
  setpriv \
    --reuid="${APP_UID}" \
    --regid="${APP_GID}" \
    --init-groups \
    -- \
    env -i \
      HOME="${HOME_DIR}" \
      USER="${APP_USER}" \
      LOGNAME="${APP_USER}" \
      SHELL=/bin/bash \
      PATH=/usr/local/bin:/usr/bin \
      LANG=en_US.UTF-8 \
      LC_ALL=en_US.UTF-8 \
      XDG_RUNTIME_DIR="${RUNTIME_DIR}" \
      XDG_CONFIG_HOME="${HOME_DIR}/.config" \
      XDG_DATA_HOME="${HOME_DIR}/.local/share" \
      XDG_CACHE_HOME="${HOME_DIR}/.cache" \
      DBUS_SESSION_BUS_ADDRESS="${DBUS_ADDRESS}" \
      PULSE_SERVER="unix:${RUNTIME_DIR}/pulse/native" \
      AMD_VULKAN_ICD=RADV \
      STEAM_RUNTIME=1 \
      "$@"
}

start_root() {
  printf 'steam-remote: starting %s\n' "$1"
  shift
  "$@" &
  children+=("$!")
}

start_user() {
  printf 'steam-remote: starting %s\n' "$1"
  shift
  user_command "$@" &
  children+=("$!")
}

start_auxiliary() {
  printf 'steam-remote: starting %s\n' "$1"
  shift
  "$@" &
  auxiliaries+=("$!")
}

wait_for_socket() {
  local path=$1

  for _ in {1..100}; do
    [[ -S "${path}" ]] && return 0
    sleep 0.1
  done

  printf 'steam-remote: %s was not created\n' "${path}" >&2
  return 1
}

write_lifecycle() {
  local state=$1
  local streaming=$2
  local game=$3
  local update=$4
  local controller=$5
  local heartbeat=$6
  local temporary=${LIFECYCLE_FILE}.$$

  printf 'state=%s\nstreaming=%s\ngame=%s\nupdate=%s\ncontroller=%s\nheartbeat=%s\n' \
    "${state}" "${streaming}" "${game}" "${update}" "${controller}" "${heartbeat}" \
    >"${temporary}"
  mv -f "${temporary}" "${LIFECYCLE_FILE}"
}

lifecycle_value() {
  local key=$1
  local missing=$2

  if [[ ! -r "${LIFECYCLE_FILE}" ]]; then
    printf '%s\n' "${missing}"
    return
  fi

  awk -F= -v key="${key}" -v missing="${missing}" '
    $1 == key { print substr($0, index($0, "=") + 1); found = 1; exit }
    END { if (!found) print missing }
  ' "${LIFECYCLE_FILE}"
}

parse_update_event() {
  local line=$1
  local payload

  PARSED_UPDATE_APPID=
  PARSED_UPDATE_ACTIVE=

  if [[ "${line}" =~ AppID[[:space:]]+([0-9]+)[[:space:]]+(state|update)[[:space:]]+changed[[:space:]]*:[[:space:]]*(.*)$ ]]; then
    PARSED_UPDATE_APPID=${BASH_REMATCH[1]}
    payload=${BASH_REMATCH[3]}
    if [[ "${payload}" =~ (^|,)[[:space:]]*(Update[[:space:]]+)?Running([,[:space:]]|$) ]]; then
      PARSED_UPDATE_ACTIVE=true
    else
      PARSED_UPDATE_ACTIVE=false
    fi
    return 0
  fi

  if [[ "${line}" =~ AppID[[:space:]]+([0-9]+)[[:space:]]+update[[:space:]]+(canceled|complete|completed|finished) ]]; then
    PARSED_UPDATE_APPID=${BASH_REMATCH[1]}
    PARSED_UPDATE_ACTIVE=false
    return 0
  fi

  return 1
}

update_set_active() {
  local app_id=$1
  local existing

  for existing in ${UPDATE_ACTIVE_IDS:-}; do
    [[ "${existing}" == "${app_id}" ]] && return
  done
  UPDATE_ACTIVE_IDS=${UPDATE_ACTIVE_IDS:+${UPDATE_ACTIVE_IDS} }${app_id}
}

update_set_idle() {
  local app_id=$1
  local existing
  local remaining=

  for existing in ${UPDATE_ACTIVE_IDS:-}; do
    if [[ "${existing}" != "${app_id}" ]]; then
      remaining=${remaining:+${remaining} }${existing}
    fi
  done
  UPDATE_ACTIVE_IDS=${remaining}
}

file_identity_size() {
  local path=$1

  if stat -Lc '%i %s' "${path}" 2>/dev/null; then
    return 0
  fi
  stat -Lf '%i %z' "${path}" 2>/dev/null
}

snapshot_content_log() {
  if read -r CONTENT_LOG_INODE CONTENT_LOG_OFFSET < <(file_identity_size "${CONTENT_LOG}"); then
    return
  fi
  CONTENT_LOG_INODE=missing
  CONTENT_LOG_OFFSET=0
}

poll_update_activity() {
  local path=$1
  local inode
  local size
  local contents
  local line

  if ! read -r inode size < <(file_identity_size "${path}"); then
    UPDATE_STATUS=unknown
    return
  fi

  if [[ "${inode}" != "${UPDATE_LOG_INODE}" || "${size}" -lt "${UPDATE_LOG_OFFSET}" ]]; then
    UPDATE_LOG_INODE=${inode}
    UPDATE_LOG_OFFSET=0
  fi

  if [[ "${size}" -gt "${UPDATE_LOG_OFFSET}" ]]; then
    if ! contents=$(tail -c "+$((UPDATE_LOG_OFFSET + 1))" "${path}" 2>/dev/null); then
      UPDATE_STATUS=unknown
      return
    fi
    while IFS= read -r line || [[ -n "${line}" ]]; do
      if parse_update_event "${line}"; then
        if [[ "${PARSED_UPDATE_ACTIVE}" == true ]]; then
          update_set_active "${PARSED_UPDATE_APPID}"
        else
          update_set_idle "${PARSED_UPDATE_APPID}"
        fi
      fi
    done <<<"${contents}"
    UPDATE_LOG_OFFSET=${size}
  fi

  if [[ -n "${UPDATE_ACTIVE_IDS:-}" ]]; then
    UPDATE_STATUS=true
  else
    UPDATE_STATUS=false
  fi
}

remote_play_connection_activity() {
  local sockets

  if ! sockets=$(ss -H -tn state established '( sport = :27036 )' 2>/dev/null); then
    printf 'unknown\n'
  elif [[ -n "${sockets}" ]]; then
    printf 'true\n'
  else
    printf 'false\n'
  fi
}

pipewire_node_ids() {
  awk '
    function emit() {
      if (id != "" && gamescope && video_source) print id
    }
    /^[[:space:]]*id [0-9]+,/ {
      emit()
      id = $2
      sub(/,$/, "", id)
      gamescope = 0
      video_source = 0
      next
    }
    /node.name[[:space:]]*=[[:space:]]*"gamescope"/ { gamescope = 1 }
    /media.class[[:space:]]*=[[:space:]]*"Video\/Source"/ { video_source = 1 }
    END { emit() }
  '
}

pipewire_node_state() {
  awk '
    /state:[[:space:]]*"running"/ { result = "true" }
    /state:[[:space:]]*"(idle|suspended)"/ { if (result == "") result = "false" }
    END { if (result == "") print "unknown"; else print result }
  '
}

gamescope_capture_activity() {
  local nodes
  local node_ids
  local node_id
  local info
  local state
  local unknown=false

  if ! nodes=$(user_command pw-cli ls Node 2>/dev/null); then
    printf 'unknown\n'
    return
  fi
  node_ids=$(printf '%s\n' "${nodes}" | pipewire_node_ids)
  if [[ -z "${node_ids}" ]]; then
    printf 'unknown\n'
    return
  fi

  for node_id in ${node_ids}; do
    if ! info=$(user_command pw-cli info "${node_id}" 2>/dev/null); then
      unknown=true
      continue
    fi
    state=$(printf '%s\n' "${info}" | pipewire_node_state)
    if [[ "${state}" == true ]]; then
      printf 'true\n'
      return
    fi
    [[ "${state}" == false ]] || unknown=true
  done

  if [[ "${unknown}" == true ]]; then
    printf 'unknown\n'
  else
    printf 'false\n'
  fi
}

stream_activity() {
  local remote_play
  local capture

  remote_play=$(remote_play_connection_activity)
  capture=$(gamescope_capture_activity)

  if [[ "${remote_play}" == true || "${capture}" == true ]]; then
    printf 'true\n'
  elif [[ "${remote_play}" == false && "${capture}" == false ]]; then
    printf 'false\n'
  else
    printf 'unknown\n'
  fi
}

game_activity() {
  local proc_root=$1
  local environment
  local variable
  local readable=0

  if [[ ! -d "${proc_root}" ]]; then
    printf 'unknown\n'
    return
  fi

  for environment in "${proc_root}"/[0-9]*/environ; do
    [[ -r "${environment}" ]] || continue
    readable=$((readable + 1))
    while IFS= read -r -d '' variable; do
      if [[ "${variable}" =~ ^Steam(AppId|GameId)=[1-9][0-9]*$ ]]; then
        printf 'true\n'
        return
      fi
    done 2>/dev/null <"${environment}" || true
  done

  if ((readable > 0)); then
    printf 'false\n'
  else
    printf 'unknown\n'
  fi
}

combine_activity() {
  local value

  for value in "$@"; do
    if [[ "${value}" == unknown ]]; then
      printf 'unknown\n'
      return
    fi
  done
  for value in "$@"; do
    if [[ "${value}" == true ]]; then
      printf 'true\n'
      return
    fi
  done
  printf 'false\n'
}

evaluate_lifecycle() {
  local activity=$1
  local quiet_since=$2
  local now=$3
  local idle_seconds=${4:-${IDLE_SECONDS}}

  if [[ "${activity}" == unknown ]]; then
    NEXT_LIFECYCLE_STATE=error
    NEXT_QUIET_SINCE=0
    NEXT_LIMITER=full
  elif [[ "${activity}" == true ]]; then
    NEXT_LIFECYCLE_STATE=active
    NEXT_QUIET_SINCE=0
    NEXT_LIMITER=full
  elif [[ "${quiet_since}" -eq 0 ]]; then
    NEXT_LIFECYCLE_STATE=waiting
    NEXT_QUIET_SINCE=${now}
    NEXT_LIMITER=full
  elif ((now - quiet_since >= idle_seconds)); then
    NEXT_LIFECYCLE_STATE=parked
    NEXT_QUIET_SINCE=${quiet_since}
    NEXT_LIMITER=parked
  else
    NEXT_LIFECYCLE_STATE=waiting
    NEXT_QUIET_SINCE=${quiet_since}
    NEXT_LIMITER=full
  fi
}

set_gamescope_limiter() {
  local limiter=$1
  local target=${FPS}

  [[ "${limiter}" == parked ]] && target=${PARKED_FPS}
  user_command env GAMESCOPE_WAYLAND_DISPLAY="${GAMESCOPE_DISPLAY}" \
    gamescopectl debug_set_fps_limit "${target}" >/dev/null 2>&1
}

deep_park_stop() {
  local name

  for name in "${DEEP_PARK_NAMES[@]}"; do
    pkill -STOP -u "${APP_UID}" -x "${name}" 2>/dev/null || true
  done
}

deep_park_resume() {
  local name

  for name in "${DEEP_PARK_NAMES[@]}"; do
    pkill -CONT -u "${APP_UID}" -x "${name}" 2>/dev/null || true
  done
}

auto_update_due() {
  local state=$1
  local quiet_since=$2
  local now=$3
  local last_update=$4

  [[ "${AUTO_UPDATE}" != 0 ]] || return 1
  [[ "${state}" == parked ]] || return 1
  ((quiet_since > 0)) || return 1
  ((now - quiet_since >= IDLE_SECONDS + AUTO_UPDATE_PARKED_SECONDS)) || return 1
  ((now - last_update >= AUTO_UPDATE_INTERVAL_SECONDS))
}

request_session_restart() {
  local now=$1

  printf '%s\n' "${now}" >"${AUTO_UPDATE_STAMP}"
  : >"${AUTO_UPDATE_MARKER}"
  printf 'steam-remote: session idle; restarting Steam to apply pending updates\n'
  deep_park_resume
  kill "${children[@]}" 2>/dev/null || true
}

activity_reasons() {
  local streaming=$1
  local game=$2
  local update=$3
  local reasons=

  [[ "${streaming}" == true ]] && reasons=stream
  [[ "${game}" == true ]] && reasons=${reasons:+${reasons},}game
  [[ "${update}" == true ]] && reasons=${reasons:+${reasons},}update
  printf '%s\n' "${reasons:-none}"
}

lifecycle_supervisor() {
  local initial_inode=$1
  local initial_offset=$2
  local lifecycle_state=active
  local last_reported_state=
  # Start unset so the first pass always pushes the limiter, otherwise a session
  # that begins active never applies the cap at all.
  local limiter=
  local limiter_healthy=true
  local next_retry=0
  local quiet_since=0
  local now
  local streaming
  local game
  local activity
  local controller
  local reasons
  local auto_update_last
  local deep_parked=false

  auto_update_last=$(cat "${AUTO_UPDATE_STAMP}" 2>/dev/null) || auto_update_last=0
  [[ "${auto_update_last}" =~ ^[0-9]+$ ]] || auto_update_last=0

  UPDATE_LOG_INODE=${initial_inode}
  UPDATE_LOG_OFFSET=${initial_offset}
  UPDATE_STATUS=unknown
  UPDATE_ACTIVE_IDS=

  while :; do
    now=$(date +%s)
    streaming=$(stream_activity)
    game=$(game_activity /proc)
    poll_update_activity "${CONTENT_LOG}"
    activity=$(combine_activity "${streaming}" "${game}" "${UPDATE_STATUS}")
    evaluate_lifecycle "${activity}" "${quiet_since}" "${now}"
    quiet_since=${NEXT_QUIET_SINCE}
    lifecycle_state=${NEXT_LIFECYCLE_STATE}

    # Wake from hibernation before touching the limiter: gamescopectl needs a
    # running Gamescope, and uncertainty must always resume the session.
    if [[ "${deep_parked}" == true && "${NEXT_LIMITER}" == full ]]; then
      deep_park_resume
      deep_parked=false
      printf 'steam-remote: session resumed from hibernation\n'
    fi

    if [[ "${NEXT_LIMITER}" == "${limiter}" ]]; then
      limiter_healthy=true
      next_retry=0
    elif ((now >= next_retry)); then
      if set_gamescope_limiter "${NEXT_LIMITER}"; then
        limiter=${NEXT_LIMITER}
        limiter_healthy=true
        next_retry=0
        if [[ "${limiter}" == parked ]]; then
          printf 'steam-remote: Gamescope parked at %s FPS\n' "${PARKED_FPS}"
        elif [[ "${activity}" == unknown ]]; then
          printf 'steam-remote: Gamescope restored to full rate after detector uncertainty\n'
        else
          reasons=$(activity_reasons "${streaming}" "${game}" "${UPDATE_STATUS}")
          printf 'steam-remote: Gamescope active (%s)\n' "${reasons}"
        fi
      else
        limiter_healthy=false
        lifecycle_state=error
        next_retry=$((now + LIFECYCLE_RETRY_SECONDS))
        printf 'steam-remote: unable to set Gamescope %s limiter; retrying\n' "${NEXT_LIMITER}" >&2
      fi
    else
      limiter_healthy=false
      lifecycle_state=error
    fi

    controller=true
    if [[ "${activity}" == unknown || "${limiter_healthy}" != true ]]; then
      controller=false
      lifecycle_state=error
    fi

    if [[ "${lifecycle_state}" != "${last_reported_state}" ]]; then
      case "${lifecycle_state}" in
        waiting) printf 'steam-remote: idle countdown started (%ss)\n' "${IDLE_SECONDS}" ;;
        active)
          reasons=$(activity_reasons "${streaming}" "${game}" "${UPDATE_STATUS}")
          printf 'steam-remote: lifecycle active (%s)\n' "${reasons}"
          ;;
        error) printf 'steam-remote: lifecycle detection or control degraded; keeping full rate\n' >&2 ;;
      esac
      last_reported_state=${lifecycle_state}
    fi

    write_lifecycle "${lifecycle_state}" "${streaming}" "${game}" "${UPDATE_STATUS}" "${controller}" "${now}"

    if auto_update_due "${lifecycle_state}" "${quiet_since}" "${now}" "${auto_update_last}"; then
      request_session_restart "${now}"
      return 0
    fi

    # Hibernate only once the parked limiter is confirmed applied, so the
    # session is in a known state when it freezes.
    if [[ "${lifecycle_state}" == parked && "${limiter}" == parked \
      && "${deep_parked}" == false ]]; then
      deep_park_stop
      deep_parked=true
      printf 'steam-remote: session hibernated; suspended idle processes\n'
    fi

    sleep 1
  done
}

mount_scratch() {
  local target=$1
  local options=$2

  # Engines premount some of these as tmpfs (Podman: /run, plus /tmp and
  # /var/tmp when read-only), and earlier in-place session restarts leave our
  # own mounts behind. Remount those in place — a fresh mount would shadow
  # submounts such as a /run/udev bind. Mode is settable only at mount time.
  if [[ "$(findmnt -no FSTYPE "${target}" 2>/dev/null)" == tmpfs ]]; then
    if mount -o "remount,${options%,mode=*}" "${target}"; then
      return 0
    fi
  fi
  if ! mount -t tmpfs -o "${options}" scratch "${target}"; then
    printf 'steam-remote: unable to mount tmpfs on %s; the container must run privileged\n' \
      "${target}" >&2
    return 1
  fi
}

prepare_runtime() {
  # The image root is read-only, so the entrypoint mounts every writable
  # runtime path itself; no --tmpfs flags are needed at run time. The sizes
  # are ceilings, not allocations: memory is only used for what is stored.
  mount_scratch /run rw,exec,nosuid,size=1g,mode=755
  mount_scratch /tmp rw,exec,nosuid,size=8g,mode=1777
  mount_scratch /var/tmp rw,exec,nosuid,size=2g,mode=1777
  mount_scratch /var/lib/xkb rw,exec,nosuid,size=64m,mode=1777

  # Remove leftovers from a previous session so daemons do not trip over
  # stale pid files and sockets after an in-place restart.
  rm -rf /run/dbus "${RUNTIME_DIR}" "${CONTROL_DIR}" /tmp/.X11-unix

  install -d -m 0755 -o "${APP_UID}" -g "${APP_GID}" "${HOME_DIR}"
  install -d -m 0755 -o "${APP_UID}" -g "${APP_GID}" "${STATE_DIR}"
  install -d -m 0755 "${CONTROL_DIR}"
  write_lifecycle error unknown unknown unknown false "$(date +%s)"

  if [[ ! -s "${STATE_DIR}/machine-id" ]]; then
    tr -d '-' </proc/sys/kernel/random/uuid >"${STATE_DIR}/machine-id"
    chmod 0444 "${STATE_DIR}/machine-id"
  fi
  if ! findmnt -no TARGET /etc/machine-id >/dev/null 2>&1; then
    mount --bind "${STATE_DIR}/machine-id" /etc/machine-id
  fi

  install -d -m 0700 -o "${APP_UID}" -g "${APP_GID}" "${RUNTIME_DIR}"
  install -d -m 0755 /run/dbus
  install -d -m 1777 /tmp/.X11-unix /var/lib/xkb

  chmod a+rw /dev/dri/* /dev/input/event* /dev/uinput /dev/uhid
}

cleanup() {
  trap - EXIT
  deep_park_resume
  if ((${#auxiliaries[@]} > 0)); then
    kill "${auxiliaries[@]}" 2>/dev/null || true
    wait "${auxiliaries[@]}" 2>/dev/null || true
  fi
  if ((${#children[@]} > 0)); then
    kill "${children[@]}" 2>/dev/null || true
    wait "${children[@]}" 2>/dev/null || true
  fi
}

run_session() {
  local content_log_inode
  local content_log_offset

  prepare_runtime
  trap 'exit 0' INT TERM
  trap cleanup EXIT

  start_root system-dbus dbus-daemon --system --nofork
  wait_for_socket /run/dbus/system_bus_socket

  start_user session-dbus dbus-daemon --session --nofork --address="${DBUS_ADDRESS}"
  wait_for_socket "${RUNTIME_DIR}/bus"

  start_user pipewire pipewire
  wait_for_socket "${RUNTIME_DIR}/pipewire-0"
  start_user wireplumber wireplumber
  start_user pipewire-pulse pipewire-pulse
  wait_for_socket "${RUNTIME_DIR}/pulse/native"

  user_command pactl load-module module-null-sink \
    sink_name=steam_stream_audio \
    sink_properties=device.description=Steam_Stream_Audio \
    rate=48000 \
    channels=8 \
    channel_map=front-left,front-right,rear-left,rear-right,front-center,lfe,side-left,side-right >/dev/null
  user_command pactl set-default-sink steam_stream_audio

  snapshot_content_log
  content_log_inode=${CONTENT_LOG_INODE}
  content_log_offset=${CONTENT_LOG_OFFSET}

  # Xwayland derives the core screen millimeters from its DPI setting at
  # startup and never updates them, so pass a DPI matching the advertised
  # physical size or Steam scales its UI and streamed cursor for 96 DPI.
  local phys_width_mm=${PHYS_MM%%x*}
  local xwayland_dpi=$(((WIDTH * 254 + phys_width_mm * 5) / (phys_width_mm * 10)))
  printf '#!/usr/bin/env bash\nexec /usr/bin/Xwayland "$@" -dpi %s\n' "${xwayland_dpi}" \
    >"${CONTROL_DIR}/xwayland"
  chmod 0755 "${CONTROL_DIR}/xwayland"

  # Gamescope's -r flag paces compositing and PipeWire capture, not the game
  # render loop: a game with vsync off presents without blocking and renders
  # past the stream rate, spending GPU on frames the capture never delivers.
  # DXVK and vkd3d-proton read these to cap Proton games at the stream rate.
  start_user gamescope \
    env \
      XDG_CURRENT_DESKTOP=gamescope \
      XDG_SESSION_DESKTOP=gamescope \
      XDG_SESSION_TYPE=wayland \
      ENABLE_GAMESCOPE_WSI=1 \
      DXVK_FRAME_RATE="${FPS}" \
      VKD3D_FRAME_RATE="${FPS}" \
      XCURSOR_SIZE="${CURSOR_CANVAS}" \
      STEAM_REMOTE_CURSOR_CANVAS="${CURSOR_CANVAS}" \
      STEAM_REMOTE_CURSOR_GLYPH="${CURSOR_GLYPH}" \
      FSR4_UPGRADE=1 \
      GAMESCOPE_HEADLESS_PHYS_MM="${PHYS_MM}" \
      GAMESCOPE_PIPEWIRE_NO_NV12="${ZERO_COPY}" \
      WLR_XWAYLAND="${CONTROL_DIR}/xwayland" \
    gamescope \
      -e \
      --rt \
      --backend headless \
      --xwayland-count 2 \
      --force-windows-fullscreen \
      -W "${WIDTH}" \
      -H "${HEIGHT}" \
      -w "${WIDTH}" \
      -h "${HEIGHT}" \
      -r "${FPS}" \
      -- \
      /usr/bin/steam -pipewire -gamepadui

  wait_for_socket "${GAMESCOPE_SOCKET}"
  start_auxiliary lifecycle lifecycle_supervisor "${content_log_inode}" "${content_log_offset}"

  printf 'steam-remote: ready at %sx%s@%s\n' "${WIDTH}" "${HEIGHT}" "${FPS}"

  set +e
  wait -n "${children[@]}"
  set -e
  if [[ -e "${AUTO_UPDATE_MARKER}" ]]; then
    rm -f "${AUTO_UPDATE_MARKER}"
    return "${SESSION_RESTART_STATUS}"
  fi
  printf 'steam-remote: a required process exited\n' >&2
  return 1
}

is_running() {
  pgrep -u "${APP_UID}" -x "$1" >/dev/null
}

report() {
  local format=${1:-text}
  local gamescope=false
  local pipewire=false
  local pulse=false
  local steam=false
  local remote_play=false
  local lifecycle=false
  local lifecycle_state
  local lifecycle_controller
  local streaming
  local game
  local update
  local heartbeat
  local now
  local healthy=true

  if is_running gamescope || is_running gamescope-wl; then
    gamescope=true
  fi
  [[ -S "${RUNTIME_DIR}/pipewire-0" ]] && pipewire=true
  [[ -S "${RUNTIME_DIR}/pulse/native" ]] && pulse=true
  if is_running steam || is_running steamwebhelper; then
    steam=true
  fi
  if ss -H -ltn 'sport = :27036' | grep -q .; then
    remote_play=true
  fi

  lifecycle_state=$(lifecycle_value state error)
  lifecycle_controller=$(lifecycle_value controller false)
  streaming=$(lifecycle_value streaming unknown)
  game=$(lifecycle_value game unknown)
  update=$(lifecycle_value update unknown)
  heartbeat=$(lifecycle_value heartbeat 0)
  now=$(date +%s)

  case "${lifecycle_state}" in
    active|waiting|parked|error) ;;
    *) lifecycle_state=error ;;
  esac
  case "${streaming}" in true|false|unknown) ;; *) streaming=unknown ;; esac
  case "${game}" in true|false|unknown) ;; *) game=unknown ;; esac
  case "${update}" in true|false|unknown) ;; *) update=unknown ;; esac
  if [[ "${lifecycle_controller}" == true && "${heartbeat}" =~ ^[0-9]+$ ]] \
    && ((heartbeat <= now && now - heartbeat <= LIFECYCLE_STALE_SECONDS)); then
    lifecycle=true
  fi

  for check in "${gamescope}" "${pipewire}" "${pulse}" "${steam}" "${remote_play}" "${lifecycle}"; do
    [[ "${check}" == true ]] || healthy=false
  done

  if [[ "${format}" == json ]]; then
    [[ "${streaming}" == unknown ]] && streaming=null
    [[ "${game}" == unknown ]] && game=null
    [[ "${update}" == unknown ]] && update=null
    printf '{"healthy":%s,"checks":{"gamescope":%s,"pipewire":%s,"pulse":%s,"steam":%s,"remote_play":%s,"lifecycle":%s},"lifecycle":{"state":"%s","controller_healthy":%s,"streaming":%s,"game_running":%s,"update_running":%s}}\n' \
      "${healthy}" "${gamescope}" "${pipewire}" "${pulse}" "${steam}" "${remote_play}" "${lifecycle}" \
      "${lifecycle_state}" "${lifecycle}" "${streaming}" "${game}" "${update}"
  else
    printf 'healthy: %s\ngamescope: %s\npipewire: %s\npulse: %s\nsteam: %s\nremote play: %s\nlifecycle: %s\nlifecycle controller: %s\nstreaming: %s\ngame running: %s\nupdate running: %s\n' \
      "${healthy}" "${gamescope}" "${pipewire}" "${pulse}" "${steam}" "${remote_play}" \
      "${lifecycle_state}" "${lifecycle}" "${streaming}" "${game}" "${update}"
  fi

  [[ "${healthy}" == true ]]
}

# Host-side image updaters call this before replacing the container: exit 0
# approves the update, 75 (EX_TEMPFAIL) tells Watchtower and systemd
# ExecCondition alike to skip and retry later. Only a parked session — no
# stream, game, or download for at least five minutes — approves.
update_gate() {
  local state
  state=$(lifecycle_value state error)

  if [[ "${AUTO_UPDATE}" != 0 && "${state}" == parked ]]; then
    return 0
  fi
  return 75
}

report_format() {
  case "${1:-}" in
    "") printf 'text\n' ;;
    --json) printf 'json\n' ;;
    *)
      printf 'usage: steam-remote %s [--json]\n' "${2}" >&2
      return 2
      ;;
  esac
}

main() {
  local command=${1:-}
  local format

  case "${command}" in
    run)
      [[ $# -eq 1 ]] || { printf 'usage: steam-remote run\n' >&2; return 2; }
      local session_status
      while :; do
        session_status=0
        run_session || session_status=$?
        [[ "${session_status}" -eq "${SESSION_RESTART_STATUS}" ]] || return "${session_status}"
        cleanup
        children=()
        auxiliaries=()
      done
      ;;
    status)
      [[ $# -le 2 ]] || { printf 'usage: steam-remote status [--json]\n' >&2; return 2; }
      format=$(report_format "${2:-}" status)
      report "${format}" || true
      ;;
    health)
      [[ $# -le 2 ]] || { printf 'usage: steam-remote health [--json]\n' >&2; return 2; }
      format=$(report_format "${2:-}" health)
      report "${format}"
      ;;
    update-gate)
      [[ $# -eq 1 ]] || { printf 'usage: steam-remote update-gate\n' >&2; return 2; }
      update_gate
      ;;
    *)
      printf 'usage: steam-remote {run|status|health|update-gate} [--json]\n' >&2
      return 2
      ;;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
