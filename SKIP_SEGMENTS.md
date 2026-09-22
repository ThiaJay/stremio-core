# Core skip segments

This branch treats skipping as a shared Core capability rather than as a provider-specific or client-specific feature.

The objective is one resilient skip-segment model for intro, recap, credits/outro and preview evidence. Every Stremio client consumes the same resolved Core state and performs only the normal player action. The feature must remain useful when any one evidence source is unavailable and must never make playback less reliable.

## Product contract

- Skip behaviour is a Core feature, not an addon playback workaround.
- Stremio native Skip Gaps remains supported as one trusted evidence source whenever its existing entitlement permits it.
- Independent evidence is queried in parallel and can establish a segment even when native Skip Gaps is unavailable.
- Native evidence is not treated as the feature and external evidence is not treated as a secondary feature. They are inputs to one provider-neutral resolver.
- The user experience is available to all clients that can consume the Core model. Client UI may follow platform conventions, including the compact Android TV style prompt, without changing resolution semantics.
- Ask, Always and Never remain the user-facing behaviour modes. Automatic skipping must still fail closed when evidence is not trustworthy.
- Intro, recap and credits/outro are first-class segment kinds. Preview data may be retained as evidence but must not be auto-skipped by default until a client contract explicitly opts in.

## Evidence sources

Stremio native Skip Gaps remains unchanged and is only requested when the existing Premium checks pass.

SkipDB is the portable external source. Its public read API is browser compatible and is used on every platform. SkipDB data is treated as transient read-only evidence and is never written into Stremio's persistent skip segment cache. SkipDB attribution and data licence information are available at https://skipdb.tv and https://skipdb.tv/data.

On non-WebAssembly clients, IntroDB and TheIntroDB provide additional independent evidence. They are not called directly from WebAssembly clients because live browser-origin testing showed that IntroDB does not allow the Stremio Web origin and TheIntroDB may be blocked by Cloudflare from automated environments.

Additional future sources may be added only through the same candidate model. Source-specific player logic is not permitted.

## Community evidence

Community-supplied timing is useful only when its quality information survives ingestion.

The candidate model therefore keeps provider confidence and evidence count separately. Resolution may accept strong standalone community evidence or independent agreement between sources. A simple provider priority list must never override stronger stream-specific or independently corroborated evidence.

Community voting, contributor reputation, clustering and reporting systems are useful upstream quality mechanisms. Core should consume their resulting confidence or evidence strength where a provider exposes it. Core should not recreate provider-specific reputation systems or couple playback to user accounts on those services.

This design was independently reinforced by public community projects that use voting, clustering and multiple sources. Those projects are evidence for requirements and failure modes only. Their code, branding and database are not incorporated unless separately reviewed for licence, API terms and attribution.

## No stream rewriting

Core skip behaviour must never modify, proxy, transcode, splice or replace the selected media stream in order to skip a segment.

A skip is a player action against the already selected playable stream.

This invariant avoids a documented class of addon failures where rewritten HLS manifests, byte-range manifests or debrid transcoding produced wrong durations, audio-only playback, player fallback or episode auto-advance. Failure to obtain or apply skip evidence must leave ordinary playback unchanged.

Consequences:

- no alternate "Skip" stream variants;
- no dependency on Real-Debrid, TorBox, Premiumize, AllDebrid or a scraper;
- no dependency on stream container support for HLS rewriting;
- no proxy requirement;
- no skip-specific playback URL;
- no skip failure may trigger next-episode navigation or watched completion.

## Failure isolation

Providers are requested independently. No external provider is awaited before playback continues and failure from one provider does not suppress evidence from another provider or Stremio native Skip Gaps.

Every provider result carries the playback generation and media identity that initiated it. Results for an earlier episode, stream context or duration are discarded after playback moves on.

Provider timeout, malformed response, HTTP failure, CORS failure, rate limiting or temporary outage must produce no player-side failure.

## Resolution

Provider responses are converted into a common candidate model before entering the player decision path.

The resolver prefers:

1. stronger stream specificity;
2. stronger match quality;
3. unadjusted evidence;
4. stronger evidence count;
5. higher provider confidence.

Provider identity itself is not a ranking criterion.

The resolver fails closed. Weak standalone external evidence is not exposed. Independent sources can establish a segment when they materially agree. Trusted peers at the same evidence level suppress the result when their timings materially conflict. Invalid ranges, tiny segments and implausibly long ranges are rejected.

SkipDB results reported as out of range or otherwise non-exact are treated as estimated evidence. They do not establish a skip segment on their own at low confidence.

External evidence only changes the segment kinds it actually supplies. Existing native values are preserved when there is no trusted alternative candidate for that kind.

For credits/outro, Core must retain enough information to distinguish "skip the remaining credits" from "play next episode". Client presentation and next-video priority must never turn an outro timing error into an unintended episode change.

## Cache and freshness

Successful cache-eligible external candidates are cached locally per provider using media identity and exact duration. IntroDB and TheIntroDB therefore retain independent outage fallbacks.

Cache entries expire after 30 days. A stale, future-dated, malformed or wrong-provider entry is rejected.

SkipDB candidates are explicitly excluded from persistent caching. This keeps the SkipDB integration within its read-only usage model rather than using its data to populate a Stremio skip database.

Cached data is an outage fallback while fresh provider reads run independently. A successful fresh result replaces only that provider's cached evidence. A failed fresh read leaves other valid provider caches untouched.

Fresh conflicting evidence must be able to withdraw a previously exposed external segment.

## Privacy and credentials

The integration sends only public media identifiers and the minimum episode or duration information required by each provider.

It must not send:

- Stremio authentication material;
- debrid credentials;
- scraper manifest URLs;
- playback URLs;
- addon credentials;
- account watch history unless a future provider contract explicitly requires it and the user separately opts in.

No shared third-party API secret is embedded in clients.

## Cross-platform behaviour

All clients consume the same resolved Core segment state, action contract, timing boundaries, dismissal rules, priority rules and user preferences.

Android TV is a visual and interaction reference for the contextual skip control, not a separate product path. The same contextual action must exist across TV, desktop, Web and mobile wherever the client can render the shared player state.

Platform-specific code must be limited to unavoidable input and rendering adaptation, such as remote focus, touch activation or pointer activation. It must not change feature availability, labels, timing semantics, evidence resolution, automatic-skip behaviour, dismissal behaviour or priority against other player UI.

WebAssembly clients may have different provider access constraints because of browser networking rules. Native clients may reach additional independent providers where their APIs and licences permit it. Those transport differences must converge into the same Core candidate model and resolved result.

The target presentation is one unified contextual player control, derived from Stremio's established Android TV skip/end interaction pattern and adapted consistently across screen sizes and input methods. It is never a permanent control-bar feature and never a separate Android-only implementation.

## Profile compatibility

The shared product setting is the skip-segment behaviour mode for intro, recap and credits.

The current serialized profile field remains named `skipIntroMode` for backward compatibility with profiles and clients created before the generic segment model. That legacy wire name must not leak into new user-facing labels or narrow the semantics to intros. New client presentation should describe the general skip capability while reading and writing the compatible profile field until a separately versioned settings migration is justified.

## Accessibility and control

- The prompt must be keyboard, remote and screen-reader reachable.
- Ask mode never steals focus from primary playback controls.
- Always mode skips only a currently active, trusted segment and only once per playback generation.
- Never mode still permits Core to resolve evidence internally but exposes no actionable seek.
- Dismissal is bound to the playback generation and exact segment.
- Manual seeking into a segment must not create an automatic seek loop.
- Seeking backwards after an automatic skip may offer the action again only under an explicit, deterministic re-entry rule.
- Next-video UI keeps priority near episode end.

## Segment contribution

A future contribution path is compatible with this design, but it is separate from playback.

Useful community patterns include segment submission, correction reports, voting and clustering. Any Stremio contribution mechanism should submit evidence to an appropriate public service or Stremio-owned endpoint rather than giving arbitrary addons permission to control the player timeline.

Contribution identity and reputation must not be required to consume resolved skip data.

## Explicit non-goals

This implementation does not copy IntroHater or any other community project's code, UI assets, branding, database or contribution system.

It does not make Core dependent on a particular addon, debrid service, scraper or community client.

It does not weaken Stremio's existing Premium entitlement around the native Skip Gaps service. It makes the general Core skip capability independent of whether that one evidence source is available.

## Validation gates

Before production promotion, validate all of the following on current Core and representative clients:

- native evidence only;
- each external source alone;
- multiple agreeing sources;
- materially conflicting sources;
- provider outage and timeout;
- stale and late result delivery;
- duration mismatch;
- malformed and boundary segments;
- intro, recap and credits/outro independently;
- Ask, Always and Never;
- manual seek into and out of a segment;
- rewind after skip;
- same episode reload;
- rapid episode transition;
- next-video popup collision;
- live playback;
- unseekable media;
- WebAssembly browser restrictions;
- native desktop;
- Android mobile;
- Android TV remote focus;
- no playback URL or stream object mutation;
- no unintended watched-state or next-episode transition.

Runtime acceptance must use a genuine running build. Synthetic screenshots are not product evidence.
