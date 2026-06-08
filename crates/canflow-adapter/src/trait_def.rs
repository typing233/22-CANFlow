use async_trait::async_trait;
use canflow_types::{CanFlowError, CanFrame, FrameFilter};
use tokio::sync::mpsc;

#[async_trait]
pub trait CanAdapter: Send {
    fn name(&self) -> &str;

    async fn run(&mut self, tx: mpsc::Sender<CanFrame>) -> Result<(), CanFlowError>;

    async fn send(&mut self, frame: &CanFrame) -> Result<(), CanFlowError>;

    fn set_filters(&mut self, filters: Vec<FrameFilter>);

    async fn shutdown(&mut self);
}
