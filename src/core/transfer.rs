use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransferItem {
    pub local_path: PathBuf,
    pub remote_path: String,
    pub direction: TransferDirection,
    pub size: u64,
    pub transferred: u64,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TransferStatus {
    Queued,
    InProgress,
    Complete,
    Failed,
}

impl TransferItem {
    #[allow(dead_code)]
    pub fn progress(&self) -> f32 {
        if self.size == 0 {
            return 1.0;
        }
        self.transferred as f32 / self.size as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_zero_size_returns_one() {
        let item = TransferItem {
            local_path: PathBuf::from("/tmp/empty"),
            remote_path: "/remote/empty".into(),
            direction: TransferDirection::Upload,
            size: 0,
            transferred: 0,
            status: TransferStatus::Complete,
        };
        assert_eq!(item.progress(), 1.0);
    }

    #[test]
    fn progress_half_transferred() {
        let item = TransferItem {
            local_path: PathBuf::from("/tmp/file"),
            remote_path: "/remote/file".into(),
            direction: TransferDirection::Download,
            size: 1000,
            transferred: 500,
            status: TransferStatus::InProgress,
        };
        assert!((item.progress() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_fully_transferred() {
        let item = TransferItem {
            local_path: PathBuf::from("/tmp/done"),
            remote_path: "/remote/done".into(),
            direction: TransferDirection::Upload,
            size: 4096,
            transferred: 4096,
            status: TransferStatus::Complete,
        };
        assert_eq!(item.progress(), 1.0);
    }

    #[test]
    fn progress_none_transferred() {
        let item = TransferItem {
            local_path: PathBuf::from("/tmp/queued"),
            remote_path: "/remote/queued".into(),
            direction: TransferDirection::Download,
            size: 1_000_000,
            transferred: 0,
            status: TransferStatus::Queued,
        };
        assert_eq!(item.progress(), 0.0);
    }

    #[test]
    fn direction_equality() {
        assert_eq!(TransferDirection::Upload, TransferDirection::Upload);
        assert_ne!(TransferDirection::Upload, TransferDirection::Download);
    }

    #[test]
    fn status_equality() {
        assert_eq!(TransferStatus::Queued, TransferStatus::Queued);
        assert_ne!(TransferStatus::Queued, TransferStatus::Complete);
        assert_ne!(TransferStatus::InProgress, TransferStatus::Failed);
    }
}
