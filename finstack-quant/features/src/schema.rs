//! Schema registry for the features crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_features_schemas` binary writes one JSON Schema per entry, and the
//! WASM package generates its TypeScript declarations from them. Render an
//! entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::schema::SchemaArtifact;

use crate::{CrossSectionalOp, PairwiseOp, PanelTransformResult, PanelTransformSpec, TimeSeriesOp};

/// The crate's complete schema registry.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        CrossSectionalOp,
        "features",
        "cross_sectional_op",
        Component,
        "Supported cross-sectional transform operation."
    ),
    finstack_quant_core::schema_artifact!(
        PairwiseOp,
        "features",
        "pairwise_op",
        Component,
        "Supported pairwise rolling time-series transform operation."
    ),
    finstack_quant_core::schema_artifact!(
        PanelTransformResult,
        "features",
        "panel_transform_result",
        Output,
        "Ordered result columns from a panel transform pipeline."
    ),
    finstack_quant_core::schema_artifact!(
        PanelTransformSpec,
        "features",
        "panel_transform_spec",
        Input,
        "Specification for a panel transform pipeline."
    ),
    finstack_quant_core::schema_artifact!(
        TimeSeriesOp,
        "features",
        "time_series_op",
        Component,
        "Supported time-series transform operation."
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_artifact_renders_a_titled_schema() {
        for artifact in ARTIFACTS {
            let schema = artifact.generate().expect("render schema");
            assert!(schema.get("title").is_some(), "{}", artifact.type_name());
        }
    }
}
