import assert from 'node:assert/strict'
import { test } from 'node:test'
import { compareVersions } from './update.ts'

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
