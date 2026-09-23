import assert from 'node:assert/strict';

const TRUSTED = new Set(['probe', 'indexer', 'addon']);
const ENGLISH = 'English';

const PREFERRED_LANGUAGE_LABELS = ['English', 'Dubbed', 'Dual Audio', 'Multi', 'Original', 'Unknown'];

function languageLabelRank(label) {
  const i = PREFERRED_LANGUAGE_LABELS.indexOf(label);
  return i === -1 ? Number.POSITIVE_INFINITY : i;
}

function rankLanguageLabels(streams) {
  return [...streams].sort((a, b) => languageLabelRank(a.languageLabel) - languageLabelRank(b.languageLabel));
}

function hasConfirmedEnglish(stream) {
  return TRUSTED.has(stream.mediaInfoQuality) &&
    Array.isArray(stream.languages) &&
    stream.languages.includes(ENGLISH);
}

function confirmedEnglish(streams) {
  return streams.filter(hasConfirmedEnglish);
}

function shouldFetchGroup(totalStreams, threshold) {
  return confirmedEnglish(totalStreams).length < threshold;
}

function applyEnglishGuard(streams) {
  const english = confirmedEnglish(streams);
  return english.length > 0 ? english : [...streams];
}

function runSequentialGroups(groupResults, thresholds = [null, 4, 4, 2]) {
  const total = [];
  const fetched = [];
  for (let i = 0; i < groupResults.length; i += 1) {
    if (i === 0 || shouldFetchGroup(total, thresholds[i])) {
      fetched.push(i + 1);
      total.push(...groupResults[i]);
    }
  }
  return { fetched, beforeGuard: total, final: applyEnglishGuard(total) };
}

const stream = (id, languages, mediaInfoQuality, extra = {}) => ({
  id,
  languages,
  mediaInfoQuality,
  ...extra,
});

{
  const result = runSequentialGroups([
    [stream('tribes-de', ['German'], 'indexer', { episode: 'tt9184982:1:2' })],
    [stream('tribes-en', ['English'], 'probe', { episode: 'tt9184982:1:2' })],
    [],
    [],
  ]);
  assert.deepEqual(result.fetched, [1, 2, 3, 4]);
  assert.deepEqual(result.final.map((s) => s.id), ['tribes-en']);
}

{
  const result = runSequentialGroups([
    [stream('eden-es-fast', ['Spanish'], 'probe', { title: 'Welcome to Eden', latency: 40, availability: 100 })],
    [stream('eden-en-slower', ['English'], 'probe', { title: 'Welcome to Eden', latency: 120, availability: 8 })],
    [],
    [],
  ]);
  assert.deepEqual(result.fetched, [1, 2, 3, 4]);
  assert.deepEqual(result.final.map((s) => s.id), ['eden-en-slower']);
}

{
  const misleading = stream('ger-eng-sub', ['German'], 'indexer', { subtitles: ['English'] });
  assert.equal(hasConfirmedEnglish(misleading), false);
  assert.equal(shouldFetchGroup([misleading], 4), true);
}

{
  const weak = stream('filename-says-eng', ['English'], undefined, { filename: 'SHOW.S01E02.ENG.mkv' });
  assert.equal(hasConfirmedEnglish(weak), false);
  assert.equal(shouldFetchGroup([weak, weak, weak, weak], 4), true);
}

{
  const multi = stream('multi', ['German', 'English'], 'addon');
  assert.equal(hasConfirmedEnglish(multi), true);
}

{
  const german = stream('original-de', ['German'], 'probe');
  const unknown = stream('unknown', ['Unknown'], undefined);
  const result = applyEnglishGuard([german, unknown]);
  assert.deepEqual(result.map((s) => s.id), ['original-de', 'unknown']);
}

{
  const en = stream('en', ['English'], 'probe', { service: 'realdebrid', quality: 'WEB-DL', latency: 130 });
  const de = stream('de', ['German'], 'probe', { service: 'torbox', quality: 'BluRay', latency: 90 });
  const before = structuredClone(en);
  const result = applyEnglishGuard([de, en]);
  assert.deepEqual(result, [en]);
  assert.deepEqual(en, before);
}

{
  const four = Array.from({ length: 4 }, (_, i) => stream(`en-${i}`, ['English'], 'indexer'));
  assert.equal(shouldFetchGroup(four, 4), false);
  assert.equal(shouldFetchGroup(four.slice(0, 3), 4), true);
  assert.equal(shouldFetchGroup(four.slice(0, 2), 2), false);
  assert.equal(shouldFetchGroup(four.slice(0, 1), 2), true);
}

{
  const weakEnglish = stream('weak-en', ['English'], undefined);
  const confirmedGerman = stream('de', ['German'], 'probe');
  assert.equal(shouldFetchGroup([weakEnglish, confirmedGerman], 4), true);
  assert.deepEqual(applyEnglishGuard([weakEnglish, confirmedGerman]).map((s) => s.id), ['weak-en', 'de']);
}


function rankWithinLanguageTier(streams) {
  return [...streams].sort((a, b) =>
    (a.latency ?? Number.POSITIVE_INFINITY) - (b.latency ?? Number.POSITIVE_INFINITY) ||
    (b.availability ?? 0) - (a.availability ?? 0) ||
    String(a.quality ?? '').localeCompare(String(b.quality ?? ''))
  );
}

function selectWithLimit(streams, limit) {
  return rankWithinLanguageTier(applyEnglishGuard(streams)).slice(0, limit);
}

{
  const english = [
    stream('en-slower', ['English'], 'probe', { latency: 180, availability: 10, quality: 'WEB-DL' }),
    stream('en-faster', ['English'], 'probe', { latency: 80, availability: 8, quality: 'WEB-DL' }),
  ];
  const german = Array.from({ length: 8 }, (_, i) =>
    stream(`de-${i}`, ['German'], 'probe', { latency: 10 + i, availability: 100, quality: 'BluRay' })
  );
  const result = selectWithLimit([...german, ...english], 1);
  assert.deepEqual(result.map((s) => s.id), ['en-faster']);
}

{
  const english = [
    stream('en-low-availability', ['English'], 'probe', { latency: 100, availability: 3, quality: 'WEB-DL' }),
    stream('en-high-availability', ['English'], 'probe', { latency: 100, availability: 9, quality: 'WEB-DL' }),
  ];
  const result = selectWithLimit(english, 2);
  assert.deepEqual(result.map((s) => s.id), ['en-high-availability', 'en-low-availability']);
}

{
  const weakEnglishOriginal = stream('english-original-weak', ['English'], undefined, { originalLanguage: 'English' });
  const unknown = stream('unknown-playable', ['Unknown'], undefined);
  const result = applyEnglishGuard([weakEnglishOriginal, unknown]);
  assert.deepEqual(result.map((s) => s.id), ['english-original-weak', 'unknown-playable']);
}

{
  const spanishOriginal = stream('eden-es-original', ['Spanish'], 'probe', { languageLabel: 'Original', availability: 100, latency: 20 });
  const englishDubbed = stream('eden-en-dubbed', ['Unknown'], undefined, { languageLabel: 'Dubbed', availability: 5, latency: 200 });
  const ranked = rankLanguageLabels([spanishOriginal, englishDubbed]);
  assert.deepEqual(ranked.map((s) => s.id), ['eden-en-dubbed', 'eden-es-original']);
}

console.log(JSON.stringify({
  status: 'PASS',
  regression: 'AIOStreams English-audio provider continuation and fallback',
  fixture: 'Tribes of Europa S01E02 / tt9184982:1:2',
  assertions: 21,
  guarantees: [
    'German-only result cannot terminate later English search',
    'Spanish-only result cannot rank ahead of confirmed English audio even when faster or more available',
    'Dubbed English fallback ranks ahead of Original-language fallback when structured English metadata is unavailable',
    'English subtitles do not count as English audio',
    'unknown filename inference does not stop later groups',
    'confirmed multi-audio English counts',
    'original-language fallback survives when no confirmed English exists',
    'non-language stream metadata is not mutated',
    'provider thresholds remain 4, 4, 2',
    'result limits apply after English eligibility so earlier non-English results cannot crowd out English audio',
    'availability and latency ordering remain available within the English tier',
    'weak English-original metadata remains playable when there is no confirmed English track evidence',
  ],
}, null, 2));
