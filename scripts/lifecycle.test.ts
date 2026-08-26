import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';

const root = `${import.meta.dir}/..`;
const runtime = `${root}/container/steam-remote.sh`;
const fixtureRoot = await mkdtemp(`${tmpdir()}/steam-remote-lifecycle-`);

const harness = String.raw`
set -euo pipefail

source "$1"
fixture_root=$2

fail() {
  printf 'lifecycle test failed: %s\n' "$1" >&2
  exit 1
}

assert_equal() {
  expected=$1
  actual=$2
  message=$3
  [[ "$actual" == "$expected" ]] || fail "$message (expected $expected, got $actual)"
}

parse_update_event '[2026-08-26] AppID 10 update changed : Running,Validating,'
assert_equal 10 "$PARSED_UPDATE_APPID" 'active update AppID'
assert_equal true "$PARSED_UPDATE_ACTIVE" 'active update state'

parse_update_event '[2026-08-26] AppID 10 state changed : Fully Installed,'
assert_equal false "$PARSED_UPDATE_ACTIVE" 'completed state clears activity'

parse_update_event '[2026-08-26] AppID 10 update canceled : User canceled'
assert_equal false "$PARSED_UPDATE_ACTIVE" 'canceled update clears activity'

if parse_update_event '[2026-08-26] unrelated content message'; then
  fail 'unrelated content line was treated as an update event'
fi

content_log="$fixture_root/content_log.txt"
UPDATE_LOG_INODE=missing
UPDATE_LOG_OFFSET=0
UPDATE_ACTIVE_IDS=
UPDATE_STATUS=unknown

printf '[2026-08-26] AppID 20 update changed : Running,Downloading,\n' >"$content_log"
poll_update_activity "$content_log"
assert_equal true "$UPDATE_STATUS" 'running update activity'

printf '[2026-08-26] AppID 20 update changed : Idle,\n' >>"$content_log"
poll_update_activity "$content_log"
assert_equal false "$UPDATE_STATUS" 'finished update activity'

replacement="$fixture_root/content_log.next"
printf '[2026-08-26] AppID 30 state changed : Update Running,Update Started,\n' >"$replacement"
mv -f "$replacement" "$content_log"
poll_update_activity "$content_log"
assert_equal true "$UPDATE_STATUS" 'update activity after log rotation'

printf '[2026-08-26] AppID 30 update completed : Success\n' >>"$content_log"
poll_update_activity "$content_log"
assert_equal false "$UPDATE_STATUS" 'completion after log rotation'

mkdir -p "$fixture_root/proc/100" "$fixture_root/proc/200"
printf 'PATH=/usr/bin\0SteamAppId=123\0' >"$fixture_root/proc/100/environ"
assert_equal true "$(game_activity "$fixture_root/proc")" 'running game process'

printf 'PATH=/usr/bin\0SteamGameId=0\0' >"$fixture_root/proc/100/environ"
printf 'PATH=/usr/bin\0' >"$fixture_root/proc/200/environ"
assert_equal false "$(game_activity "$fixture_root/proc")" 'idle process tree'
assert_equal unknown "$(game_activity "$fixture_root/missing-proc")" 'missing process detector'

evaluate_lifecycle false 0 100 300
assert_equal waiting "$NEXT_LIFECYCLE_STATE" 'quiet period begins waiting'
assert_equal 100 "$NEXT_QUIET_SINCE" 'quiet period start time'
assert_equal full "$NEXT_LIMITER" 'waiting keeps full rate'

evaluate_lifecycle false 100 399 300
assert_equal waiting "$NEXT_LIFECYCLE_STATE" 'quiet period remains waiting'

evaluate_lifecycle false 100 400 300
assert_equal parked "$NEXT_LIFECYCLE_STATE" 'quiet period parks at timeout'
assert_equal parked "$NEXT_LIMITER" 'parked limiter target'

evaluate_lifecycle true 100 401 300
assert_equal active "$NEXT_LIFECYCLE_STATE" 'activity wakes lifecycle'
assert_equal 0 "$NEXT_QUIET_SINCE" 'activity resets quiet period'
assert_equal full "$NEXT_LIMITER" 'activity restores full rate'

evaluate_lifecycle unknown 100 401 300
assert_equal error "$NEXT_LIFECYCLE_STATE" 'unknown activity fails safe'
assert_equal full "$NEXT_LIMITER" 'unknown activity keeps full rate'
assert_equal unknown "$(combine_activity false unknown false)" 'unknown detector precedence'

node_fixture='id 42, type PipeWire:Interface:Node/3
    node.name = "gamescope"
    media.class = "Video/Source"'
assert_equal 42 "$(printf '%s\n' "$node_fixture" | pipewire_node_ids)" 'Gamescope PipeWire node parsing'
assert_equal true "$(printf 'state: "running"\n' | pipewire_node_state)" 'running PipeWire state'
assert_equal false "$(printf 'state: "suspended"\n' | pipewire_node_state)" 'idle PipeWire state'

printf 'Lifecycle fixture checks passed.\n'
`;

try {
  const result = Bun.spawnSync({
    cmd: ['bash', '-c', harness, 'lifecycle-test', runtime, fixtureRoot],
    cwd: root,
    stdout: 'inherit',
    stderr: 'inherit',
  });

  if (!result.success) {
    process.exitCode = result.exitCode;
  }
} finally {
  await rm(fixtureRoot, { recursive: true, force: true });
}
