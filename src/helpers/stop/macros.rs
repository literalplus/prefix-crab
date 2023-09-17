#[macro_export]
macro_rules! loop_with_stop {

(fn_param $work_ident:ident on it) => {
    $work_ident
};
(fn_param $_:ident on $work_arg:ident) => {
    $work_arg
};
(fn_param $_:ident on (&$work_arg:ident)) => {
    &$work_arg
};

(fn_call $($calls:ident).+($($params:tt),*) with $work_arg:ident) => {
    $($calls).+($(loop_with_stop!(fn_param $work_arg on $params)),*)
};

(recv $task_name:expr, $stop_rx:ident, $work_rx:ident => $($calls:ident).+($($params:tt),*)) => {
    loop_with_stop!($task_name, $stop_rx, $work_rx.recv() => $($calls).+($($params),*) as result_async)
};

($task_name:expr, $stop_rx:ident, $work_rx:ident.$op:ident() => $($calls:ident).+($($params:tt),*) as $result_type:ident) => {
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
                loop_with_stop!($result_type work_opt for $task_name, $($calls).+($($params),*))
            }
        }
    }
};

(result_async $result_opt:ident for $task_name:expr, $($calls:ident).+($($params:tt),*)) => {
    if let Some(work) = $result_opt {
        loop_with_stop!(fn_call $($calls).+($($params),*) with work).await?;
    } else {
        log::debug!("Sender closed channel for {}", $task_name);
        return anyhow::Result::Ok(());
    }
};

(simple $result_simple:ident for $task_name:expr, $($calls:ident).+($($params:tt),*)) => {
    loop_with_stop!(fn_call $($calls).+($($params),*) with $result_simple)
};

(simple_async $result_simple:ident for $task_name:expr, $($calls:ident).+(it$(, $work_arg:ident)*)) => {
    loop_with_stop!(fn_call $($calls).+($($params),*) with $result_simple).await?;
}
}
