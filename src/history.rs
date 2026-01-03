use std::collections::VecDeque;

/// 状态历史记录
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateHistory {
    history: VecDeque<String>,
    max_size: usize,
}

impl StateHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// 推送一个状态到历史记录中
    pub fn push(&mut self, state: impl Into<String>) {
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(state.into());
    }

    /// 获取状态历史记录
    pub fn get_history(&self) -> Vec<&str> {
        self.history.iter().map(|s| s.as_str()).collect()
    }

    /// 获取当前状态
    pub fn get_current(&self) -> Option<&str> {
        self.history.back().map(|s| s.as_str())
    }

    /// 获取上一个状态
    pub fn get_previous(&self) -> Option<&str> {
        if self.history.len() < 2 {
            None
        } else {
            self.history.iter().rev().nth(1).map(|s| s.as_str())
        }
    }

    /// 获取指定索引的历史状态 (0是当前状态，1是上一个状态，等等)
    pub fn get_at(&self, index: usize) -> Option<&str> {
        if index < self.history.len() {
            self.history.iter().rev().nth(index).map(|s| s.as_str())
        } else {
            None
        }
    }

    /// 清除历史记录（保留当前状态）
    pub fn clear_history_except_current(&mut self) {
        if let Some(current) = self.history.back().cloned() {
            self.history.clear();
            self.history.push_back(current);
        }
    }

    /// 获取历史记录长度
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// 检查历史记录是否为空
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

impl Default for StateHistory {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(10),
            max_size: 10,
        }
    }
}
