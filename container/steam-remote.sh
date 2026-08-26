#!/usr/bin/env bash
set -euo pipefail

readonly APP_USER=steam
readonly APP_UID=1000
readonly APP_GID=1000
readonly HOME_DIR=/mnt/data
readonly STATE_DIR=${HOME_DIR}/.config/steam-remote
readonly RUNTIME_DIR=/run/user/${APP_UID}
readonly DBUS_ADDRESS=unix:path=${RUNTIME_DIR}/bus
readonly WIDTH=${STEAM_REMOTE_WIDTH:-3840}
readonly HEIGHT=${STEAM_REMOTE_HEIGHT:-2160}
readonly FPS=${STEAM_REMOTE_FPS:-60}

declare -a children=()

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

wait_for_socket() {
  local path=$1

  for _ in {1..100}; do
    [[ -S "${path}" ]] && return 0
    sleep 0.1
  done

  printf 'steam-remote: %s was not created\n' "${path}" >&2
  return 1
}

prepare_runtime() {
  install -d -m 0755 -o "${APP_UID}" -g "${APP_GID}" "${HOME_DIR}"
  install -d -m 0755 -o "${APP_UID}" -g "${APP_GID}" "${STATE_DIR}"

  if [[ ! -s "${STATE_DIR}/machine-id" ]]; then
    tr -d '-' </proc/sys/kernel/random/uuid >"${STATE_DIR}/machine-id"
    chmod 0444 "${STATE_DIR}/machine-id"
  fi
  mount --bind "${STATE_DIR}/machine-id" /etc/machine-id

  install -d -m 0700 -o "${APP_UID}" -g "${APP_GID}" "${RUNTIME_DIR}"
  install -d -m 0755 /run/dbus
  install -d -m 1777 /tmp/.X11-unix /var/lib/xkb

  chmod a+rw /dev/dri/* /dev/input/event* /dev/uinput /dev/uhid
}

cleanup() {
  trap - EXIT
  if ((${#children[@]} > 0)); then
    kill "${children[@]}" 2>/dev/null || true
    wait "${children[@]}" 2>/dev/null || true
  fi
}

run_session() {
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
    channels=2 \
    channel_map=front-left,front-right >/dev/null
  user_command pactl set-default-sink steam_stream_audio

  start_user gamescope \
    env \
      XDG_CURRENT_DESKTOP=gamescope \
      XDG_SESSION_DESKTOP=gamescope \
      XDG_SESSION_TYPE=wayland \
      ENABLE_GAMESCOPE_WSI=1 \
    gamescope \
      -e \
      --backend headless \
      --force-grab-cursor \
      -W "${WIDTH}" \
      -H "${HEIGHT}" \
      -w "${WIDTH}" \
      -h "${HEIGHT}" \
      -r "${FPS}" \
      -- \
      /usr/bin/steam -gamepadui

  printf 'steam-remote: ready at %sx%s@%s\n' "${WIDTH}" "${HEIGHT}" "${FPS}"

  set +e
  wait -n "${children[@]}"
  set -e
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
  local healthy=true

  is_running gamescope && gamescope=true
  [[ -S "${RUNTIME_DIR}/pipewire-0" ]] && pipewire=true
  [[ -S "${RUNTIME_DIR}/pulse/native" ]] && pulse=true
  if is_running steam || is_running steamwebhelper; then
    steam=true
  fi
  if ss -H -ltn 'sport = :27036' | grep -q .; then
    remote_play=true
  fi

  for check in "${gamescope}" "${pipewire}" "${pulse}" "${steam}" "${remote_play}"; do
    [[ "${check}" == true ]] || healthy=false
  done

  if [[ "${format}" == json ]]; then
    printf '{"healthy":%s,"checks":{"gamescope":%s,"pipewire":%s,"pulse":%s,"steam":%s,"remote_play":%s}}\n' \
      "${healthy}" "${gamescope}" "${pipewire}" "${pulse}" "${steam}" "${remote_play}"
  else
    printf 'healthy: %s\ngamescope: %s\npipewire: %s\npulse: %s\nsteam: %s\nremote play: %s\n' \
      "${healthy}" "${gamescope}" "${pipewire}" "${pulse}" "${steam}" "${remote_play}"
  fi

  [[ "${healthy}" == true ]]
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
      run_session
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
    *)
      printf 'usage: steam-remote {run|status|health} [--json]\n' >&2
      return 2
      ;;
  esac
}

main "$@"
