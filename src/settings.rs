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
    FunctionFit,
}

impl OutputMode {
    pub(crate) const ALL: [Self; 5] = [
        Self::Auto,
        Self::Lines,
        Self::Bezier,
        Self::Mixed,
        Self::FunctionFit,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Lines => "Lines",
            Self::Bezier => "Bezier",
            Self::Mixed => "Mixed",
            Self::FunctionFit => "Function fit",
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
}
