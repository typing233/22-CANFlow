use canflow_analysis::Alert;
use canflow_bus::LiveStatsSnapshot;
use canflow_types::CanFrame;
use std::collections::VecDeque;
use std::sync::Arc;

pub struct App {
    pub frames: VecDeque<Arc<CanFrame>>,
    pub alerts: VecDeque<Alert>,
    pub stats: Option<LiveStatsSnapshot>,
    pub max_frames: usize,
    pub max_alerts: usize,
    pub paused: bool,
    pub selected_tab: usize,
    pub filter_input: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            frames: VecDeque::with_capacity(1000),
            alerts: VecDeque::with_capacity(100),
            stats: None,
            max_frames: 1000,
            max_alerts: 100,
            paused: false,
            selected_tab: 0,
            filter_input: String::new(),
            should_quit: false,
        }
    }

    pub fn push_frame(&mut self, frame: Arc<CanFrame>) {
        if self.paused {
            return;
        }
        if self.frames.len() >= self.max_frames {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }

    pub fn push_alert(&mut self, alert: Alert) {
        if self.alerts.len() >= self.max_alerts {
            self.alerts.pop_front();
        }
        self.alerts.push_back(alert);
    }

    pub fn update_stats(&mut self, stats: LiveStatsSnapshot) {
        self.stats = Some(stats);
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % 3;
    }

    pub fn prev_tab(&mut self) {
        if self.selected_tab == 0 {
            self.selected_tab = 2;
        } else {
            self.selected_tab -= 1;
        }
    }
}
