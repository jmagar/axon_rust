use super::*;
use tokio::sync::OwnedSemaphorePermit;

impl TeiClient {
    pub(super) async fn acquire_admission(
        &self,
        inputs: usize,
    ) -> Result<
        (
            OwnedSemaphorePermit,
            OwnedSemaphorePermit,
            OwnedSemaphorePermit,
        ),
        ApiError,
    > {
        let count = u32::try_from(inputs).map_err(|_| {
            self.error(
                "embedding.tei.input_budget_invalid",
                "TEI client batch exceeds the weighted admission range",
            )
        })?;
        let profile = self
            .profile_input_slots
            .clone()
            .acquire_many_owned(count)
            .await
            .map_err(|_| {
                self.error(
                    "embedding.tei.admission_closed",
                    "TEI invocation input admission gate is closed",
                )
            })?;
        let request = self
            .request_slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                self.error(
                    "embedding.tei.admission_closed",
                    "TEI request admission gate is closed",
                )
            })?;
        let inputs = self
            .input_slots
            .clone()
            .acquire_many_owned(count)
            .await
            .map_err(|_| {
                self.error(
                    "embedding.tei.admission_closed",
                    "TEI input admission gate is closed",
                )
            })?;
        Ok((profile, request, inputs))
    }
}
