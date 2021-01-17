mod pool_state {
    use std::{collections::VecDeque, sync::Mutex};

    use futures::channel::oneshot;

    #[async_trait::async_trait]
    pub trait PoolValueManager<T, E> {
        fn is_alive(&self) -> bool;
        fn is_value_alive(&self, value: &T) -> bool;
        async fn connect(&self) -> Result<T, E>;
    }

    struct PoolStateInner<T> {
        max_values: usize,
        min_values: usize,
        max_pooled_values: usize,
        num_live_values: usize,
        pooled_values: Vec<T>,
        waiters: VecDeque<oneshot::Sender<T>>,
    }

    impl<T> PoolStateInner<T> {}

    pub struct PoolState<T, E> {
        factory: Box<dyn PoolValueManager<T, E> + Send + Sync>,
        inner: Mutex<PoolStateInner<T>>,
    }

    impl<T, E> PoolState<T, E> {
        pub async fn new<PF: PoolValueManager<T, E> + Send + Sync + 'static>(
            factory: PF,
            max_values: usize,
            min_values: usize,
            max_pooled_values: usize,
        ) -> Result<Self, E> {
            let mut pooled_values = Vec::new();
            for _ in 0..min_values {
                pooled_values.push(factory.connect().await?)
            }

            Ok(PoolState {
                factory: Box::new(factory),
                inner: Mutex::new(PoolStateInner {
                    max_values,
                    min_values,
                    max_pooled_values,
                    num_live_values: min_values,
                    pooled_values,
                    waiters: VecDeque::new(),
                }),
            })
        }

        pub async fn take_value(&self) -> Result<T, E> {
            enum InnerTakeResult<T> {
                CreateNew,
                WaitForReturn(oneshot::Receiver<T>),
            }

            let result = {
                let mut inner = self.inner.lock().unwrap();

                // Try to take a new value from the pool
                if let Some(value) = inner.pooled_values.pop() {
                    return Ok(value);
                }

                // No existing pooled value is available. Check to see if we can create a new value.

                if inner.num_live_values < inner.max_values {
                    inner.num_live_values += 1;
                    InnerTakeResult::CreateNew
                } else {
                    let (send, recv) = oneshot::channel();
                    inner.waiters.push_back(send);
                    InnerTakeResult::WaitForReturn(recv)
                }
            };

            match result {
                InnerTakeResult::CreateNew => self.factory.connect().await,
                InnerTakeResult::WaitForReturn(recv) => {
                    match recv.await {
                        Ok(value) => Ok(value),
                        Err(_) => {
                            // This means that the pool was dropped before we got our connection.
                            // We should return an error here. Alternately, we should try again.
                            panic!("Pool dropped before waiters could be validated")
                        }
                    }
                }
            }
        }

        pub fn return_value(&self, mut value: T) {
            let mut inner = self.inner.lock().unwrap();
            if inner.pooled_values.len() >= inner.max_pooled_values {
                inner.num_live_values -= 1;
                drop(value);
                return;
            }

            loop {
                match inner.waiters.pop_front() {
                    Some(waiter) => {
                        // Someone is waiting for a value. Try to send it. If it fails (because the
                        // other side was dropped) then move on to the next waiter
                        if let Err(ret_value) = waiter.send(value) {
                            value = ret_value
                        } else {
                            break;
                        }
                    }
                    None => {
                        // No waiters left. Return it to the pool.
                        inner.pooled_values.push(value);
                        break;
                    }
                }
            }
        }
    }
}

use std::sync::{Arc, Mutex};

struct ChannelPoolFactory {
    conn: lapin::Connection,
}

#[async_trait::async_trait]
impl pool_state::PoolValueManager<lapin::Channel, lapin::Error> for ChannelPoolFactory {
    async fn connect(&self) -> Result<lapin::Channel, lapin::Error> {
        self.conn.create_channel().await
    }

    fn is_alive(&self) -> bool {
        self.conn.status().connected()
    }

    fn is_value_alive(&self, value: &lapin::Channel) -> bool {
        value.status().connected()
    }
}

type ChannelPoolState = pool_state::PoolState<lapin::Channel, lapin::Error>;

pub struct ConnectionPool(Mutex<Arc<ChannelPoolState>>);

impl ConnectionPool {
    async fn take_channel(&self) -> Result<Arc<Channel>, lapin::Error> {
        
        todo!()
    }
}

pub struct Channel {
    pool_state: Arc<ChannelPoolState>,
    channel: lapin::Channel,
}
