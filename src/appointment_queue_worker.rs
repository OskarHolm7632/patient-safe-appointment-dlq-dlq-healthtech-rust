use patient_safe_appointment_dlq::{
    appointment_failure::{classify_failure, AppointmentJob, FailureDisposition},
    infrai_queue::{InfraiQueue, QueueError},
};
use std::{future::Future, pin::Pin, task::{Context, Poll, RawWaker, RawWakerVTable, Waker}};
use thiserror::Error;

#[derive(Debug, Error)]
enum WorkerError {
    #[error(transparent)]
    Queue(#[from] QueueError),
}

fn main() -> Result<(), WorkerError> { block_on(run_once()) }

async fn run_once() -> Result<(), WorkerError> {
    let queue = InfraiQueue::from_env()?;
    for message in queue.consume::<AppointmentJob>().await? {
        match classify_failure(&message.payload, 3) {
            FailureDisposition::Retry => println!("appointment job retained for retry: {}", message.payload.appointment_id),
            FailureDisposition::ManualReview(notice) => {
                let key = format!("appointment-review-{}-{}", notice.appointment_id, notice.failed_attempts);
                queue.publish(&notice, &key).await?; // queue.publish
                queue.ack(&message.message_id).await?;
                println!("manual review queued for appointment {}", notice.appointment_id);
            }
        }
    }
    Ok(())
}

fn block_on<F: Future>(future: F) -> F::Output {
    fn raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker { raw_waker() }
        fn no_op(_: *const ()) {}
        RawWaker::new(std::ptr::null(), &RawWakerVTable::new(clone, no_op, no_op, no_op))
    }
    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match Pin::as_mut(&mut future).poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
