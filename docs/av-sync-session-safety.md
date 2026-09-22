# Session bound playback recovery

Protocol version 2 keeps supervisory recovery in Core whilst normal synchronisation remains the native player's responsibility. The previous protocol remains separate during migration. New native adapters must never advertise the old corrective command.

Core assigns a new session identity on every player load. Pause, seek and end events revoke pending recovery. An observation from a previous session, previous native epoch, duplicate sample, reversed clock or expired capture cannot create a new request. Unknown timing is represented as unknown rather than stable zero.

Persistence requires at least three seconds of fresh measurements with consistent direction. Sample count alone cannot trigger intervention. Core prefers a native clock correction when genuinely supported. A same position reseek requires a residual difference of at least 250 milliseconds. These are conservative engineering thresholds rather than promises about human perception.

A session has at most one native clock attempt and one reseek attempt. Thirty seconds separate attempts. Missing or rejected acknowledgement exhausts the controller rather than creating an indefinite recovery loop. An acknowledgement records dispatch acceptance only. Stable recovery requires a subsequent two second window of fresh stable measurements and stable video.

The native adapter must repeat the session, epoch, freshness, capability and attempt checks immediately before sending any command. Playback health recovery takes priority over sync repair. Decoding problems and video stutter are not repaired by accumulating audio delay adjustments.

## Acknowledgement ordering

A clock correction or reseek can advance the native timing epoch before its acknowledgement reaches Core. That native transition must not erase the outstanding request or cancel its expiry timer. Core preserves the request identity until acknowledgement, expiry or an explicit user interruption. A late receipt can still match the original request whilst fresh measurements belong to the new native epoch. Receipt acceptance still does not mean stable playback has been achieved.

The software round trip exposed this race in Web workflow `35695153171`. The missing acknowledgement scenario remained in monitoring instead of stopping recovery. Core now has regression tests for native transitions before receipt, missing receipt expiry and immediate user cancellation. Corrected validation workflow `35695267654` passed the full Core tests, Clippy and native build before saving the generated source.

## Evidence and boundaries

Initial candidate validation run `35693512239` passed full Core tests, Clippy and the native build before generated integration files were saved. The source is based on PR 22 at `ffb9fff23f8663ca3c90e939cfba26ad841ca2c2`, preserving the existing DTS recovery work.

Tests include stale and duplicate input, clock reversal, sign changes, sample gaps, missing acknowledgement, interruption, repeated recovery and four hour simulated stable timelines. Those timelines are policy tests, not frame presentation tests or physical lip synchronisation measurements.

The first corrective adapter targets MPV and is tracked in ThiaJay/stremio-video#1. It may change the current file's clock policy or request one guarded same position seek. It must not change user audio delay, playback speed, selected tracks or source identity. Browser playback with no separate native clock measurement remains unchanged. Official Android TV integration remains outstanding and no unrelated community Android client is substituted for it.

No production deployment is authorised by a passing software test alone.
