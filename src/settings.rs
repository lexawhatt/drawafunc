#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QualityPreset {
    Rough,
    Default,
    Smooth,
    Precise,
}

impl QualityPreset {
    pub(crate) const ALL: [Self; 4] = [Self::Rough, Self::Default, Self::Smooth, Self::Precise];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Rough => "Rough",
            Self::Default => "Default",
            Self::Smooth => "Smooth",
            Self::Precise => "Precise",
        }
    }

    pub(crate) fn smoothing_passes(self) -> usize {
        match self {
            Self::Rough => 3,
            Self::Default => 1,
            Self::Smooth => 4,
            Self::Precise => 0,
        }
    }

    pub(crate) fn tolerance_multiplier(self) -> f32 {
        match self {
            Self::Rough => 2.6,
            Self::Default => 1.0,
            Self::Smooth => 1.35,
            Self::Precise => 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputMode {
    Auto,
    Lines,
    Bezier,
    Mixed,
    ExperimentalPolynomial,
    ExperimentalExponential,
}

impl OutputMode {
    pub(crate) const ALL: [Self; 6] = [
        Self::Auto,
        Self::Lines,
        Self::Bezier,
        Self::Mixed,
        Self::ExperimentalPolynomial,
        Self::ExperimentalExponential,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Lines => "Lines",
            Self::Bezier => "Bezier",
            Self::Mixed => "Mixed",
            Self::ExperimentalPolynomial => "Experimental polynomial",
            Self::ExperimentalExponential => "Experimental exponential",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GenerationSettings {
    pub(crate) quality: QualityPreset,
    pub(crate) output_mode: OutputMode,
    pub(crate) simplify_tolerance: f32,
    pub(crate) polynomial_degree: usize,
}

impl GenerationSettings {
    pub(crate) fn effective_tolerance(self) -> f32 {
        (self.simplify_tolerance * self.quality.tolerance_multiplier()).max(0.001)
    }

    pub(crate) fn function_fit_error_tolerance(self) -> f32 {
        let preset_floor: f32 = match self.quality {
            QualityPreset::Rough => 0.45,
            QualityPreset::Default => 0.22,
            QualityPreset::Smooth => 0.16,
            QualityPreset::Precise => 0.08,
        };
        preset_floor.max(self.effective_tolerance() * 2.0)
    }

    pub(crate) fn function_fit_degree_cap(self) -> usize {
        self.polynomial_degree.clamp(1, 3)
    }
}
