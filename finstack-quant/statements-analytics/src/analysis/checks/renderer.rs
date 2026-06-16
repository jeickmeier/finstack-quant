//! Report renderers that turn a [`CheckReport`] into human-readable output.
//!
//! Two formats are provided:
//!
//! - **Plain text** — suitable for terminal / log output.
//! - **HTML** — basic inline-styled HTML suitable for Jupyter notebook display.

use finstack_quant_statements::checks::{CheckReport, Severity};

/// Stateless renderer for [`CheckReport`].
pub struct CheckReportRenderer;

impl CheckReportRenderer {
    /// Render the report as plain text with severity sections.
    ///
    /// Layout:
    /// ```text
    /// ══ Check Report ══
    /// 5 checks: 3 passed, 2 failed | 1 error, 2 warnings, 1 info
    ///
    /// ── ERRORS ──
    /// [ERROR] Balance Sheet Articulation — 2025Q1
    ///   Balance sheet does not articulate …
    ///   Materiality: 50.00 (2.50% of total_assets)
    ///   Nodes: total_assets, total_liabilities, total_equity
    /// …
    /// ```
    pub fn render_text(report: &CheckReport) -> String {
        let s = &report.summary;
        let mut out = String::with_capacity(1024);

        out.push_str("══ Check Report ══\n");
        out.push_str(&format!(
            "{} checks: {} passed, {} failed | {} errors, {} warnings, {} infos\n",
            s.total_checks, s.passed, s.failed, s.errors, s.warnings, s.infos,
        ));

        for (severity, header) in &[
            (Severity::Error, "ERRORS"),
            (Severity::Warning, "WARNINGS"),
            (Severity::Info, "INFO"),
        ] {
            let findings = report.findings_by_severity(*severity);
            if findings.is_empty() {
                continue;
            }

            out.push_str(&format!("\n── {header} ──\n"));
            for f in &findings {
                let badge = match f.severity {
                    Severity::Error => "[ERROR]",
                    Severity::Warning => "[WARN] ",
                    Severity::Info => "[INFO] ",
                };

                let period_str = f
                    .period
                    .as_ref()
                    .map_or(String::new(), |p| format!(" — {p}"));

                out.push_str(&format!("{badge} {}{period_str}\n", f.check_id));
                out.push_str(&format!("  {}\n", f.message));

                if let Some(ref m) = f.materiality {
                    out.push_str(&format!(
                        "  Materiality: {:.2} ({:.2}% of {})\n",
                        m.absolute, m.relative_pct, m.reference_label,
                    ));
                }

                if !f.nodes.is_empty() {
                    let node_list: Vec<&str> = f.nodes.iter().map(|n| n.as_str()).collect();
                    out.push_str(&format!("  Nodes: {}\n", node_list.join(", ")));
                }
            }
        }

        out
    }

    /// Render the report as basic HTML with inline styles.
    ///
    /// Suitable for display in Jupyter notebooks or embedded reports.
    pub fn render_html(report: &CheckReport) -> String {
        let s = &report.summary;
        let mut out = String::with_capacity(2048);

        out.push_str("<div style=\"font-family:sans-serif;font-size:14px;\">\n");
        out.push_str("<h2 style=\"border-bottom:2px solid #333;\">Check Report</h2>\n");
        out.push_str(&format!(
            "<p><strong>{}</strong> checks: \
             <span style=\"color:green\">{} passed</span>, \
             <span style=\"color:red\">{} failed</span> &mdash; \
             {} errors, {} warnings, {} infos</p>\n",
            s.total_checks, s.passed, s.failed, s.errors, s.warnings, s.infos,
        ));

        for (severity, header, color) in &[
            (Severity::Error, "Errors", "#d32f2f"),
            (Severity::Warning, "Warnings", "#f57c00"),
            (Severity::Info, "Info", "#1976d2"),
        ] {
            let findings = report.findings_by_severity(*severity);
            if findings.is_empty() {
                continue;
            }

            out.push_str(&format!(
                "<h3 style=\"color:{color};\">{header}</h3>\n<ul>\n"
            ));

            for f in &findings {
                let period_str = f
                    .period
                    .as_ref()
                    .map_or(String::new(), |p| format!(" &mdash; {p}"));

                out.push_str(&format!(
                    "<li><strong>{}</strong>{period_str}<br/>{}\n",
                    f.check_id, f.message,
                ));

                if let Some(ref m) = f.materiality {
                    out.push_str(&format!(
                        "<br/><em>Materiality: {:.2} ({:.2}% of {})</em>\n",
                        m.absolute, m.relative_pct, m.reference_label,
                    ));
                }

                if !f.nodes.is_empty() {
                    let node_list: Vec<&str> = f.nodes.iter().map(|n| n.as_str()).collect();
                    out.push_str(&format!(
                        "<br/><small>Nodes: {}</small>\n",
                        node_list.join(", ")
                    ));
                }

                out.push_str("</li>\n");
            }

            out.push_str("</ul>\n");
        }

        out.push_str("</div>\n");
        out
    }
}
