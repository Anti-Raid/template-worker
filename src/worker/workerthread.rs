/// WorkerThread provides a simple thread implementation in which a ``Worker`` runs in its own thread with messages
/// sent to it over a channel
#[derive(Clone)]
pub struct WorkerThread {

}

// Assert that WorkerThread is Send + Sync + Clone
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<WorkerThread>();
};
