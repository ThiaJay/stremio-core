# Skip segment provider integration

This branch adds provider neutral skip segment evidence while preserving Stremio's existing native Skip Gaps entitlement rules.

## Sources

Stremio native Skip Gaps remains unchanged and is only requested when the existing Premium checks pass.

IntroDB is queried anonymously using its public segments API for TV episodes. Requests use IMDb identity plus season and episode only. Its published API terms explicitly permit media player integrations and commercial applications with reasonable API usage. No shared application API key is required for reads.

TheIntroDB is queried anonymously using its public media endpoint. No shared application API key is included or required for reads.

## Failure isolation

IntroDB and TheIntroDB are requested independently. Neither provider is awaited before playback continues and failure from one provider does not suppress evidence from another provider or Stremio native Skip Gaps.

Every provider result carries the playback context that initiated it. Results for an earlier episode or duration are discarded after playback moves on.

## Resolution

Provider responses are converted into a common candidate model before entering the player decision path. The resolver prefers more stream specific evidence, then better match quality, unadjusted evidence, stronger community evidence and provider confidence.

The resolver fails closed. Weak standalone external evidence is not exposed. Independent sources can establish a segment when they materially agree. Trusted peers at the same evidence level suppress the result when their timings materially conflict. Invalid ranges, tiny segments and implausibly long intro ranges are rejected.

External evidence only changes the segment kinds it actually supplies. Existing native intro or outro values are preserved when external providers have no trusted candidate for that kind.

## Cache

Successful external candidates are cached locally using IMDb identity, season, episode and exact duration. Cache entries expire after 30 days.

Cached data is available as an outage fallback while fresh provider reads run independently. A successful fresh result for a provider takes precedence over cached evidence for that provider. Failed fresh reads leave a valid cached fallback available.

## Privacy and credentials

The integration sends only public media identifiers and the minimum episode or duration information required by each provider. It does not send Stremio authentication material to either external provider and does not embed shared provider API keys.


## Provider selection

An earlier implementation considered SkipDB as one of the independent sources. The integration now uses IntroDB and TheIntroDB instead. This preserves independent redundancy while avoiding the additional ODbL service provider reciprocity obligations attached to SkipDB data. Stremio native Skip Gaps remains a third source when the existing Premium entitlement permits it.
