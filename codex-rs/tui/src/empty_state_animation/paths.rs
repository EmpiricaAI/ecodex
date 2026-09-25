//! Mark geometry for the fresh-conversation and onboarding logo.
//!
//! ecodex: upstream morphs the Codex mark into the OpenAI mark. ecodex renders
//! the Empirica "E" mark instead (the vector form of the block "E" the welcome
//! screen showed before the upstream renderer replaced ASCII frames), and uses
//! it for both morph endpoints so the rotation never turns into another brand.

pub(super) const EMPIRICA: (&str, [f64; 4]) = (
    "M3 2H17V5.2H7V8.4H14.5V11.6H7V14.8H17V18H3Z",
    [3.0, 2.0, 17.0, 18.0],
);
