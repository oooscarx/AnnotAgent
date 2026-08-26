//! External model usage, exact costs, and budgets.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    Actual,
    Estimated,
    Mock,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub source: UsageSource,
}

impl TokenUsage {
    #[must_use]
    pub fn known(input_tokens: u64, output_tokens: u64, source: UsageSource) -> Self {
        Self {
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            total_tokens: Some(input_tokens.saturating_add(output_tokens)),
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AdditionalUsage {
    pub image_count: u64,
    pub request_count: u64,
    pub credits: Option<Decimal>,
    pub compute_seconds: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingConfig {
    pub currency: String,
    pub input_per_million_tokens: Decimal,
    pub output_per_million_tokens: Decimal,
    pub per_image: Decimal,
    pub per_request: Decimal,
    pub per_credit: Decimal,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            currency: "USD".to_owned(),
            input_per_million_tokens: Decimal::ZERO,
            output_per_million_tokens: Decimal::ZERO,
            per_image: Decimal::ZERO,
            per_request: Decimal::ZERO,
            per_credit: Decimal::ZERO,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CostBreakdown {
    pub input_tokens: Decimal,
    pub output_tokens: Decimal,
    pub images: Decimal,
    pub requests: Decimal,
    pub credits: Decimal,
    pub total: Decimal,
}

impl PricingConfig {
    #[must_use]
    pub fn calculate(&self, tokens: &TokenUsage, additional: &AdditionalUsage) -> CostBreakdown {
        let million = Decimal::from(1_000_000_u64);
        let input = Decimal::from(tokens.input_tokens.unwrap_or(0)) * self.input_per_million_tokens
            / million;
        let output = Decimal::from(tokens.output_tokens.unwrap_or(0))
            * self.output_per_million_tokens
            / million;
        let images = Decimal::from(additional.image_count) * self.per_image;
        let requests = Decimal::from(additional.request_count) * self.per_request;
        let credits = additional.credits.unwrap_or(Decimal::ZERO) * self.per_credit;
        CostBreakdown {
            input_tokens: input,
            output_tokens: output,
            images,
            requests,
            credits,
            total: input + output + images + requests + credits,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub provider: String,
    pub model: String,
    pub endpoint_summary: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub tokens: TokenUsage,
    pub additional: AdditionalUsage,
    pub request_id: Option<String>,
    pub cost: CostBreakdown,
    pub success: bool,
    pub retry_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub requests: u64,
    pub images: u64,
    pub cost: Decimal,
}

impl UsageTotals {
    pub fn add(&mut self, record: &UsageRecord) {
        self.input_tokens = self
            .input_tokens
            .saturating_add(record.tokens.input_tokens.unwrap_or(0));
        self.output_tokens = self
            .output_tokens
            .saturating_add(record.tokens.output_tokens.unwrap_or(0));
        self.total_tokens = self
            .total_tokens
            .saturating_add(record.tokens.total_tokens.unwrap_or(0));
        self.requests = self
            .requests
            .saturating_add(record.additional.request_count);
        self.images = self.images.saturating_add(record.additional.image_count);
        self.cost += record.cost.total;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Budget {
    pub max_input_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub max_total_tokens: Option<u64>,
    pub max_cost: Option<Decimal>,
    pub max_requests: Option<u64>,
    pub max_images: Option<u64>,
    pub max_wall_clock_seconds: Option<u64>,
}

impl Budget {
    #[must_use]
    pub fn exceeded_by(&self, usage: &UsageTotals) -> Option<String> {
        let checks = [
            self.max_input_tokens
                .filter(|limit| usage.input_tokens >= *limit)
                .map(|limit| {
                    format!(
                        "input token budget reached ({}/{limit})",
                        usage.input_tokens
                    )
                }),
            self.max_output_tokens
                .filter(|limit| usage.output_tokens >= *limit)
                .map(|limit| {
                    format!(
                        "output token budget reached ({}/{limit})",
                        usage.output_tokens
                    )
                }),
            self.max_total_tokens
                .filter(|limit| usage.total_tokens >= *limit)
                .map(|limit| {
                    format!(
                        "total token budget reached ({}/{limit})",
                        usage.total_tokens
                    )
                }),
            self.max_requests
                .filter(|limit| usage.requests >= *limit)
                .map(|limit| format!("request budget reached ({}/{limit})", usage.requests)),
            self.max_images
                .filter(|limit| usage.images >= *limit)
                .map(|limit| format!("image budget reached ({}/{limit})", usage.images)),
        ];
        checks.into_iter().flatten().next().or_else(|| {
            self.max_cost
                .filter(|limit| usage.cost >= *limit)
                .map(|limit| format!("cost budget reached ({}/{limit})", usage.cost))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn cost_uses_exact_decimal_arithmetic() {
        let pricing = PricingConfig {
            input_per_million_tokens: Decimal::from_str("1.25").expect("decimal"),
            output_per_million_tokens: Decimal::from_str("4.5").expect("decimal"),
            ..PricingConfig::default()
        };
        let cost = pricing.calculate(
            &TokenUsage::known(1_000_000, 500_000, UsageSource::Actual),
            &AdditionalUsage::default(),
        );
        assert_eq!(cost.total, Decimal::from_str("3.5").expect("decimal"));
    }

    #[test]
    fn budget_boundary_stops_new_calls() {
        let budget = Budget {
            max_requests: Some(2),
            ..Budget::default()
        };
        let usage = UsageTotals {
            requests: 2,
            ..UsageTotals::default()
        };
        assert!(budget.exceeded_by(&usage).is_some());
    }
}
