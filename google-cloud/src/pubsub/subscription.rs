use std::collections::{HashMap, VecDeque};

use chrono::Duration;

use crate::pubsub::api;
use crate::pubsub::{Client, Error, Message};

/// Represents the subscription's configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionConfig {
    pub(crate) ack_deadline_duration: Duration,
    pub(crate) message_retention_duration: Option<Duration>,
    pub(crate) labels: HashMap<String, String>,
}

impl SubscriptionConfig {
    /// Set the message acknowledgement duration.
    pub fn ack_deadline(mut self, duration: Duration) -> SubscriptionConfig {
        self.ack_deadline_duration = duration;
        self
    }

    /// Enable message retention and set its duration.
    pub fn retain_messages(mut self, duration: Duration) -> SubscriptionConfig {
        self.message_retention_duration = Some(duration);
        self
    }

    /// Attach a label to the subscription.
    pub fn label(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> SubscriptionConfig {
        self.labels.insert(name.into(), value.into());
        self
    }
}

impl Default for SubscriptionConfig {
    fn default() -> SubscriptionConfig {
        SubscriptionConfig {
            ack_deadline_duration: Duration::seconds(10),
            message_retention_duration: None,
            labels: HashMap::new(),
        }
    }
}

/// Represents a subscription, tied to a topic.
#[derive(Clone)]
pub struct Subscription {
    pub(crate) client: Client,
    pub(crate) name: String,
    pub(crate) buffer: VecDeque<api::ReceivedMessage>,
}

impl Subscription {
    pub(crate) fn new(client: Client, name: impl Into<String>) -> Subscription {
        Subscription {
            client,
            name: name.into(),
            buffer: VecDeque::new(),
        }
    }

    /// Receive the next message from the subscription.
    pub async fn receive(&mut self) -> Option<Message> {
        loop {
            if let Some(handle) = self.buffer.pop_front() {
                let message = handle.message.unwrap();
                let timestamp = message.publish_time.unwrap();
                let message = Message {
                    client: self.client.clone(),
                    subscription_name: self.name.clone(),
                    data: message.data,
                    message_id: message.message_id,
                    ack_id: handle.ack_id,
                    attributes: message.attributes,
                    publish_time: chrono::NaiveDateTime::from_timestamp(
                        timestamp.seconds,
                        timestamp.nanos as u32,
                    ),
                };
                break Some(message);
            } else {
                let response = self.pull().await;
                if let Ok(messages) = response {
                    self.buffer.extend(messages);
                }
            }
        }
    }

    /// Delete the subscription.
    pub async fn delete(mut self) -> Result<(), Error> {
        let request = api::DeleteSubscriptionRequest {
            subscription: format!(
                "projects/{0}/subscriptions/{1}",
                self.client.project_name.as_str(),
                self.name,
            ),
        };
        self.client.subscriber.delete_subscription(request).await?;

        Ok(())
    }

    pub(crate) async fn pull(&mut self) -> Result<Vec<api::ReceivedMessage>, Error> {
        let request = api::PullRequest {
            subscription: format!(
                "projects/{0}/subscriptions/{1}",
                self.client.project_name.as_str(),
                self.name,
            ),
            return_immediately: false,
            max_messages: 5,
        };
        let response = self.client.subscriber.pull(request).await?;
        let response = response.into_inner();

        Ok(response.received_messages)
    }
}

// impl<'a> Stream for Subscription<'a> {
//     type Item = Message<'a>;
//     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         let fut = match self.fut {
//             Some(fut) => fut.as_mut(),
//             None => {
//                 self.fut.replace(Box::pin(self.next_message()));
//                 self.fut.as_mut().unwrap().as_mut()
//             }
//         };

//         fut.poll(cx)
//     }
// }
