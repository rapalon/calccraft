// Copies CalcCraft's web pages into a checkout of rapalon/website (website/README.md, "Sync").
//
//   node website/sync.mjs <path to a rapalon/website checkout>
//
// Writes, in that checkout:
//   src/content/projects/calccraft.md and src/content/projects/calccraft/*.png  (this folder, verbatim)
//   src/content/policies/calccraft-privacy.md  (PRIVACY.md with the `policies` front matter)
//   src/content/policies/calccraft-terms.md    (TERMS.md with the `policies` front matter)
// The front matter records the commit of this repository the texts come from (HEAD), so run it on
// a clean checkout of the commit to publish. Node only, no dependencies; it changes nothing here.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = process.argv[2];
if (!site) {
  console.error('usage: node website/sync.mjs <path to a rapalon/website checkout>');
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..');
const git = (...args) => execFileSync('git', ['-C', repo, ...args], { encoding: 'utf8' }).trim();
if (git('status', '--porcelain', '--', 'website', 'PRIVACY.md', 'TERMS.md') !== '') {
  console.error('website/, PRIVACY.md or TERMS.md has uncommitted changes: commit them first.');
  process.exit(1);
}
const commit = git('rev-parse', 'HEAD');
const lf = (text) => text.replace(/\r\n/g, '\n');

// The product page and its images.
const projects = join(site, 'src/content/projects');
mkdirSync(join(projects, 'calccraft'), { recursive: true });
copyFileSync(join(here, 'calccraft.md'), join(projects, 'calccraft.md'));
for (const file of readdirSync(join(here, 'calccraft'))) {
  copyFileSync(join(here, 'calccraft', file), join(projects, 'calccraft', file));
}

// The legal texts: the root files' text below the front matter of the `policies` collection.
const policies = join(site, 'src/content/policies');
mkdirSync(policies, { recursive: true });
for (const [kind, file, title] of [
  ['privacy', 'PRIVACY.md', 'CalcCraft Privacy Policy'],
  ['terms', 'TERMS.md', 'CalcCraft Terms of Service'],
]) {
  const front = [
    '---',
    'product: calccraft',
    `kind: ${kind}`,
    `title: ${title}`,
    `# Synced from ${file} in https://github.com/rapalon/calccraft by website/sync.mjs; edit it there.`,
    `source: https://github.com/rapalon/calccraft/blob/main/${file}`,
    `sourceCommit: ${commit}`,
    '---',
    '',
  ].join('\n');
  writeFileSync(join(policies, `calccraft-${kind}.md`), front + lf(readFileSync(join(repo, file), 'utf8')));
}
console.log(`Synced CalcCraft's pages at ${commit} into ${site}`);
