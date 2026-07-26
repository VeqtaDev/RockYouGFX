import assert from 'node:assert/strict'
import { test } from 'node:test'
import { compareVersions, parseChecksums } from './update.ts'

test('ordonne les versions numériquement, pas lexicographiquement', () => {
  // Le piège d'une comparaison de chaînes : "0.10.0" < "0.9.0" en lexicographique.
  assert.ok(compareVersions('0.10.0', '0.9.0') > 0)
  assert.ok(compareVersions('1.0.0', '0.99.99') > 0)
  assert.ok(compareVersions('2.0.0', '10.0.0') < 0)
})

test('accepte le préfixe v des tags git', () => {
  assert.equal(compareVersions('v1.2.3', '1.2.3'), 0)
  assert.ok(compareVersions('v1.2.4', '1.2.3') > 0)
})

test('tolère les composants manquants', () => {
  assert.equal(compareVersions('1.2', '1.2.0'), 0)
  assert.ok(compareVersions('1.3', '1.2.9') > 0)
})

test('ignore le suffixe de préversion', () => {
  // Le projet publie ses premières versions en préversion : elles doivent
  // rester comparables à la version embarquée dans le binaire.
  assert.equal(compareVersions('1.0.0-beta.1', '1.0.0'), 0)
  assert.ok(compareVersions('1.0.1-beta.1', '1.0.0') > 0)
})

test('une version identique ne déclenche pas de mise à jour', () => {
  assert.equal(compareVersions('0.1.0', '0.1.0'), 0)
})

test('extrait une empreinte du format sha256sum', () => {
  const sums = [
    'aa'.repeat(32) + '  RockYouGFX-v0.1.2-portable.exe',
    'bb'.repeat(32) + '  RockYouGFX-v0.1.2-setup.exe',
  ].join('\n')
  assert.equal(parseChecksums(sums, 'RockYouGFX-v0.1.2-portable.exe'), 'aa'.repeat(32))
  assert.equal(parseChecksums(sums, 'RockYouGFX-v0.1.2-setup.exe'), 'bb'.repeat(32))
})

test('gère le préfixe étoile du mode binaire', () => {
  const sums = 'cc'.repeat(32) + ' *RockYouGFX-portable.exe'
  assert.equal(parseChecksums(sums, 'RockYouGFX-portable.exe'), 'cc'.repeat(32))
})

test('renvoie null pour un fichier absent', () => {
  // Sans empreinte, la mise à jour doit échouer plutôt que de remplacer
  // le binaire sans rien vérifier.
  assert.equal(parseChecksums('aa'.repeat(32) + '  autre.exe', 'RockYouGFX.exe'), null)
  assert.equal(parseChecksums('', 'RockYouGFX.exe'), null)
})

test('ignore les lignes malformées', () => {
  const sums = ['pas une empreinte  RockYouGFX.exe', 'dd'.repeat(32) + '  RockYouGFX.exe'].join('\n')
  assert.equal(parseChecksums(sums, 'RockYouGFX.exe'), 'dd'.repeat(32))
})
