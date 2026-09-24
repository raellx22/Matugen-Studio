import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const catalog = JSON.parse(readFileSync(resolve(root, 'src-tauri/resources/matugen-themes/catalog.json'), 'utf8'));
const templates = resolve(root, 'src-tauri/resources/matugen-themes/templates');

test('curated applications expose distinct installable variant targets', () => {
  const applicationIds = new Set();
  const variantIds = new Set();
  for (const app of catalog.applications) {
    assert.ok(app.id && app.name && app.category);
    assert.ok(!applicationIds.has(app.id), `duplicate application ${app.id}`);
    applicationIds.add(app.id);
    const outputs = new Set();
    for (const variant of app.variants) {
      assert.ok(!variantIds.has(variant.id), `duplicate variant ${variant.id}`);
      variantIds.add(variant.id);
      assert.ok(existsSync(resolve(templates, variant.sourcePath)), `missing ${variant.sourcePath}`);
      const targetIds = new Set();
      for (const target of variant.targets) {
        assert.ok(target.id && target.outputPath, `incomplete target for ${variant.id}`);
        assert.ok(!targetIds.has(target.id), `duplicate target ${variant.id}/${target.id}`);
        targetIds.add(target.id);
        assert.ok(existsSync(resolve(templates, target.sourcePath ?? variant.sourcePath)), `missing input for ${variant.id}/${target.id}`);
        assert.ok(!outputs.has(target.outputPath), `variants would overwrite ${target.outputPath}`);
        outputs.add(target.outputPath);
      }
    }
  }
  assert.equal(catalog.applications.find(app => app.id === 'discord')?.variants.length, 3);
  assert.equal(catalog.applications.find(app => app.id === 'vscode')?.variants.length, 2);
});

test('Discord Material You exposes isolated Equibop Native and Flatpak destinations', () => {
  const discord = catalog.applications.find(app => app.id === 'discord');
  assert.ok(discord);
  const material = discord.variants.find(variant => variant.id === 'discord.material');
  assert.ok(material);
  for (const id of ['equibop-native', 'equibop-flatpak']) {
    const target = material.targets.find(candidate => candidate.id === id);
    assert.ok(target, `missing Material You ${id}`);
    assert.equal(target.installType, id.endsWith('flatpak') ? 'flatpak' : 'native');
    assert.ok(target.outputPath.endsWith('/equibop/themes/matugen-material.css'));
    for (const other of discord.variants.filter(variant => variant.id !== material.id)) {
      const corresponding = other.targets.find(candidate => candidate.id === id);
      assert.ok(corresponding, `missing ${other.id}/${id}`);
      assert.notEqual(target.outputPath, corresponding.outputPath);
    }
  }
});

test('bundled community variants carry independent provenance', () => {
  const community = catalog.applications.flatMap(app => app.variants).filter(variant => variant.source?.kind === 'community');
  assert.deepEqual(community.map(variant => variant.id).sort(), ['discord.material', 'vscode.material-premium']);
  for (const variant of community) {
    const { repository, author, license, licenseStatus, attribution } = variant.source;
    assert.ok(repository?.startsWith('https://') && author && license && licenseStatus && attribution, `missing provenance for ${variant.id}`);
    assert.equal(licenseStatus, 'original-work');
  }
});
