const root = `${import.meta.dir}/..`;
const failures: string[] = [];

function absolute(file: string): string {
  return `${root}/${file}`;
}

async function exists(file: string): Promise<boolean> {
  return Bun.file(absolute(file)).exists();
}

function fail(message: string): void {
  failures.push(message);
}

function run(command: string, args: string[]): void {
  const result = Bun.spawnSync({
    cmd: [command, ...args],
    cwd: root,
    stdout: 'inherit',
    stderr: 'inherit',
  });

  if (!result.success) {
    fail(`${command} exited with status ${result.exitCode}`);
  }
}

const requiredFiles = [
  'Containerfile',
  'README.md',
  'container/steam-remote.sh',
  'package.json',
  '.agents/skills/steam-remote-image-maintenance/SKILL.md',
  '.agents/skills/steam-remote-runtime-validation/SKILL.md',
];

for (const file of requiredFiles) {
  const source = Bun.file(absolute(file));
  if (!(await source.exists()) || source.size === 0) {
    fail(`Required file is missing or empty: ${file}`);
  }
}

const removedPaths = [
  'Makefile',
  'build',
  'deploy',
  'package-lock.json',
  'scripts/build.mjs',
  'scripts/check.mjs',
  'container/steam-remote',
];

for (const removedPath of removedPaths) {
  if (await exists(removedPath)) {
    fail(`Removed path still exists: ${removedPath}`);
  }
}

for (const directory of ['container', 'scripts']) {
  const entries = new Bun.Glob('*').scanSync({
    cwd: absolute(directory),
    onlyFiles: true,
  });

  for (const entry of entries) {
    if (!entry.includes('.')) {
      fail(`Source file has no extension: ${directory}/${entry}`);
    }
  }
}

if (await exists('container/steam-remote.sh')) {
  run('bash', ['-n', 'container/steam-remote.sh']);

  const shellcheck = Bun.spawnSync({
    cmd: ['shellcheck', '--version'],
    stdout: 'ignore',
    stderr: 'ignore',
  });
  if (shellcheck.success) {
    run('shellcheck', ['container/steam-remote.sh']);
  } else {
    console.warn('ShellCheck is unavailable; skipped shell linting.');
  }
}

const scannedFiles = [
  'Containerfile',
  'README.md',
  'AGENTS.md',
  'package.json',
  '.github/workflows/container.yml',
  'container/steam-remote.sh',
  'scripts/build.ts',
  '.agents/skills/steam-remote-image-maintenance/SKILL.md',
  '.agents/skills/steam-remote-runtime-validation/SKILL.md',
];
const removedConcepts = /cargo|rust|node\.js|setup-node|node-version|npm|package-lock|x11vnc|sunshine|moonlight|wolf|fallback|quadlet|systemctl|kwin|\/mnt\/user_data|\/home\/steam|\bretro\b/i;

for (const file of scannedFiles) {
  if (!(await exists(file))) continue;
  const lines = (await Bun.file(absolute(file)).text()).split('\n');
  lines.forEach((line, index) => {
    if (removedConcepts.test(line)) {
      fail(`Removed concept in ${file}:${index + 1}: ${line.trim()}`);
    }
  });
}

for (const skill of requiredFiles.filter((file) => file.endsWith('/SKILL.md'))) {
  if (!(await exists(skill))) continue;
  const contents = await Bun.file(absolute(skill)).text();
  if (!contents.startsWith('---\n') || !/^name: .+$/m.test(contents) || !/^description: .+$/m.test(contents)) {
    fail(`Skill frontmatter is incomplete: ${skill}`);
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`error: ${failure}`);
  process.exit(1);
}

console.log('Checks passed.');
