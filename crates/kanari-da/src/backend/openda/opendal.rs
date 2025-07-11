// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::backend::openda::adapter::{AdapterSubmitStat, OpenDAAdapter};
use async_trait::async_trait;
use kanari_config::da_config::OpenDAScheme;
use kanari_types::da::segment::SegmentID;
use opendal::layers::{LoggingLayer, RetryLayer};
use opendal::{Operator, Scheme};
use std::collections::HashMap;
use std::time::Duration;

pub(crate) const BACK_OFF_MIN_DELAY: Duration = Duration::from_millis(300);

pub(crate) struct OpenDalAdapter {
    stats: AdapterSubmitStat,
    operator: Operator,
    scheme: OpenDAScheme,
}

impl OpenDalAdapter {
    pub(crate) async fn new(
        scheme: OpenDAScheme,
        scheme_config: HashMap<String, String>,
        max_retries: usize,
        stats: AdapterSubmitStat,
    ) -> anyhow::Result<Self> {
        let mut op = opendal::Operator::via_iter(Scheme::from(scheme.clone()), scheme_config)?;
        op = op
            .layer(
                RetryLayer::new()
                    .with_max_times(max_retries)
                    .with_min_delay(BACK_OFF_MIN_DELAY),
            )
            .layer(LoggingLayer::default());
        op.check().await?;
        Ok(OpenDalAdapter {
            stats,
            operator: op,
            scheme,
        })
    }

    async fn submit(&self, segment_id: SegmentID, segment_bytes: &[u8]) -> anyhow::Result<()> {
        let path = segment_id.to_string();
        let mut w = self.operator.writer(&path).await?;
        w.write(segment_bytes.to_vec()).await?;
        w.close().await?;
        Ok(())
    }
}

#[async_trait]
impl OpenDAAdapter for OpenDalAdapter {
    async fn submit_segment(
        &self,
        segment_id: SegmentID,
        segment_bytes: &[u8],
        is_last_segment: bool,
    ) -> anyhow::Result<()> {
        match self.submit(segment_id, segment_bytes).await {
            Ok(_) => {
                tracing::info!("Submitted segment: {} to {:?}", segment_id, self.scheme,);
                self.stats
                    .add_done_segment(segment_id, is_last_segment)
                    .await;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}
