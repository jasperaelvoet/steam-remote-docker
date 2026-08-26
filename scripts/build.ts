const root = `${import.meta.dir}/..`;
const engine = Bun.env.CONTAINER_ENGINE ?? 'podman';
const image = Bun.env.IMAGE ?? 'localhost/steam-remote-docker:latest';

const revisionResult = Bun.spawnSync({
  cmd: ['git', 'rev-parse', '--verify', 'HEAD'],
  cwd: root,
  stdout: 'pipe',
  stderr: 'ignore',
});
if (!revisionResult.success) {
  console.error('Unable to resolve the Git revision.');
  process.exit(revisionResult.exitCode);
}
const revision = revisionResult.stdout.toString().trim();

const result = Bun.spawnSync({
  cmd: [
    engine,
    'build',
    '--platform',
    'linux/amd64',
    '--build-arg',
    `VCS_REF=${revision}`,
    '--tag',
    image,
    '--file',
    'Containerfile',
    '.',
  ],
  cwd: root,
  stdout: 'inherit',
  stderr: 'inherit',
});

if (!result.success) {
  console.error(`Image build failed with ${engine}.`);
}

process.exit(result.exitCode);
