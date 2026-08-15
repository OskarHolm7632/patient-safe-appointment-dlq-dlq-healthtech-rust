use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppointmentJob {
    pub appointment_id: String,
    pub patient_reference: String,
    pub channel: NotificationChannel,
    pub failed_attempts: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Sms,
    Voice,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalNotice {
    pub appointment_id: String,
    pub patient_reference: String,
    pub action: String,
    pub failed_attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureDisposition {
    Retry,
    ManualReview(OperationalNotice),
}

pub fn classify_failure(job: &AppointmentJob, attempt_limit: u8) -> FailureDisposition {
    let failed_attempts = job.failed_attempts.saturating_add(1);
    if failed_attempts < attempt_limit {
        return FailureDisposition::Retry;
    }

    FailureDisposition::ManualReview(OperationalNotice {
        appointment_id: job.appointment_id.clone(),
        patient_reference: job.patient_reference.clone(),
        action: "verify contact route before the appointment".to_owned(),
        failed_attempts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_failure_requests_review_without_adding_clinical_context() {
        let job = AppointmentJob {
            appointment_id: "apt-4821".to_owned(),
            patient_reference: "patient-ref-73".to_owned(),
            channel: NotificationChannel::Sms,
            failed_attempts: 2,
        };

        let FailureDisposition::ManualReview(notice) = classify_failure(&job, 3) else {
            panic!("expected manual review");
        };
        assert_eq!(notice.appointment_id, "apt-4821");
        assert_eq!(notice.patient_reference, "patient-ref-73");
        assert_eq!(notice.failed_attempts, 3);
        assert_eq!(notice.action, "verify contact route before the appointment");
    }
}
