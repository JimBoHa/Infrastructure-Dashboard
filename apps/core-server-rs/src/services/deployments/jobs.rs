use chrono::Utc;
use std::collections::HashMap;
use std::sync::MutexGuard;

use super::types::{DeploymentJob, DeploymentNodeInfo, DeploymentStep, JobStatus, StepStatus};
use super::util::trim_logs;
use super::DeploymentManager;

impl DeploymentManager {
    pub(super) fn jobs_lock(&self) -> MutexGuard<'_, HashMap<String, DeploymentJob>> {
        match self.jobs.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("Recovering deployment job store from poisoned lock");
                poisoned.into_inner()
            }
        }
    }

    pub(super) fn set_job_status(&self, job_id: &str, status: JobStatus, error: Option<String>) {
        let mut store = self.jobs_lock();
        let Some(job) = store.get_mut(job_id) else {
            return;
        };
        job.status = status;
        let now = Utc::now().to_rfc3339();
        if matches!(status, JobStatus::Running) {
            job.started_at = Some(now.clone());
        }
        if matches!(status, JobStatus::Success | JobStatus::Failed) {
            job.finished_at = Some(now);
        }
        if let Some(error) = error {
            job.error = Some(error);
        }
    }

    pub(super) fn set_job_outcome(&self, job_id: &str, outcome: Option<String>) {
        let mut store = self.jobs_lock();
        let Some(job) = store.get_mut(job_id) else {
            return;
        };
        job.outcome = outcome;
    }

    pub(super) fn job_outcome(&self, job_id: &str) -> Option<String> {
        let store = self.jobs_lock();
        store.get(job_id).and_then(|job| job.outcome.clone())
    }

    pub(super) fn start_step(&self, job_id: &str, step_name: &str) {
        self.update_step(job_id, step_name, |step| {
            step.status = StepStatus::Running;
            step.started_at = Some(Utc::now().to_rfc3339());
        });
    }

    pub(super) fn finish_step(&self, job_id: &str, step_name: &str) {
        self.update_step(job_id, step_name, |step| {
            step.status = StepStatus::Completed;
            step.finished_at = Some(Utc::now().to_rfc3339());
        });
    }

    pub(super) fn fail_step(&self, job_id: &str, step_name: &str, message: &str) {
        self.update_step(job_id, step_name, |step| {
            step.status = StepStatus::Failed;
            step.finished_at = Some(Utc::now().to_rfc3339());
            if !message.trim().is_empty() {
                step.logs.push(message.to_string());
            }
        });
    }

    pub(super) fn fail_running_steps(&self, job_id: &str, message: &str) {
        let mut store = self.jobs_lock();
        let Some(job) = store.get_mut(job_id) else {
            return;
        };
        for step in &mut job.steps {
            if step.status == StepStatus::Running {
                step.status = StepStatus::Failed;
                step.finished_at = Some(Utc::now().to_rfc3339());
                if !message.trim().is_empty() {
                    step.logs.push(message.to_string());
                    step.logs = trim_logs(step.logs.clone());
                }
            }
        }
    }

    fn update_step<F: FnOnce(&mut DeploymentStep)>(
        &self,
        job_id: &str,
        step_name: &str,
        update: F,
    ) {
        let mut store = self.jobs_lock();
        let Some(job) = store.get_mut(job_id) else {
            return;
        };
        let Some(step) = job.steps.iter_mut().find(|step| step.name == step_name) else {
            return;
        };
        update(step);
        step.logs = trim_logs(step.logs.clone());
    }

    pub(super) fn log_step(&self, job_id: &str, step_name: &str, message: &str) {
        if message.trim().is_empty() {
            return;
        }
        let mut store = self.jobs_lock();
        let Some(job) = store.get_mut(job_id) else {
            return;
        };
        let Some(step) = job.steps.iter_mut().find(|step| step.name == step_name) else {
            return;
        };
        step.logs.push(message.to_string());
        step.logs = trim_logs(step.logs.clone());
    }

    pub(super) fn update_node_info<F: FnOnce(&mut DeploymentNodeInfo)>(
        &self,
        job_id: &str,
        update: F,
    ) {
        let mut store = self.jobs_lock();
        let Some(job) = store.get_mut(job_id) else {
            return;
        };
        if job.node.is_none() {
            job.node = Some(DeploymentNodeInfo::default());
        }
        if let Some(node) = job.node.as_mut() {
            update(node);
        }
    }
}
