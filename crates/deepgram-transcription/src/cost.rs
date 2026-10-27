use crate::RequestOptions;

/// Published prerecorded pay-as-you-go rates, verified at deepgram.com/pricing.
/// Negotiated rates and unsupported model estimates remain unknown.
pub fn estimate(duration: f64, channels: u32, options: &RequestOptions) -> Option<String> {
    if !duration.is_finite() || duration < 0.0 {
        return None;
    }
    let model = options.effective_model().to_ascii_lowercase();
    let mut rate = if model.starts_with("nova-3") {
        if options.language.trim().eq_ignore_ascii_case("multi") {
            0.0052
        } else {
            0.0043
        }
    } else if model.starts_with("whisper") {
        0.0048
    } else {
        return None;
    };
    if !options.keyterms().is_empty() {
        rate += 0.0013;
    }
    Some(format!(
        "${:.3}",
        duration / 60.0 * f64::from(channels.max(1)) * rate
    ))
}
