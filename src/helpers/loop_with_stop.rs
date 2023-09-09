#[macro_export]
macro_rules! loop_with_stop {
    (recv $task_name:expr, $stop_rx:ident, $work_rx:ident => $work:ident(it)$( on $self:ident)?) => {
        loop_with_stop!($task_name, $stop_rx, $work_rx.recv() => $work(it)$( on $self)? as result)
    };

    ($task_name:expr, $stop_rx:ident, $work_rx:ident.$op:ident() => $work:ident(it)$( on $self:ident)? as $result_type:ident) => {
        loop {
            let work_fut = $work_rx.$op();
            let stop_fut = $stop_rx.cancelled();

            tokio::select! {
                biased; // Stop should take prio
                _ = stop_fut => {
                    log::trace!("Cancellation signal received by {}.", $task_name);
                    return anyhow::Result::Ok(());
                }
                work_opt = work_fut => {
                    loop_with_stop!($result_type work_opt for $task_name, $work(it)$( on $self)?)
                }
            }
        }
    };

    (result $result_opt:ident for $task_name:expr, $work:ident(it)$( on $self:ident)?) => {
        if let Some(work) = $result_opt {
            $($self.)?$work(work).await?;
        } else {
            log::debug!("Sender closed channel for {}", $task_name);
            return anyhow::Result::Ok(());
        }
    };

    (simple $result_simple:ident for $task_name:expr, $work:ident(it)$( on $self:ident)?) => {
        $($self.)?$work($result_simple).await?;
    }
}
