#[macro_export]
macro_rules! loop_recv_with_stop {
    ($task_name:expr, $stop_rx:ident, $work_rx:ident => $self:ident.$work:ident(it)) => {
        loop {
            let work_fut = $work_rx.recv();
            let stop_fut = $stop_rx.cancelled();

            tokio::select! {
                biased; // Stop should take prio
                _ = stop_fut => {
                    log::trace!("Cancellation signal received by {}.", $task_name);
                    return Ok(());
                }
                work_opt = work_fut => {
                    if let Some(work) = work_opt {
                        $self.$work(work).await?;
                    } else {
                        log::debug!("Sender closed channel for {}", $task_name);
                        return Ok(());
                    }
                }
            }
        }
    };
}
