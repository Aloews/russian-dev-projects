use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

const MAX_TASKS: usize = 250;
const MAX_DEPENDENCIES_PER_TASK: usize = 250;
const MAX_ESTIMATE_HOURS: f64 = 1_000_000.0;

#[derive(Debug, Clone, Deserialize)]
pub struct RecoveryRequest {
    pub planned_remaining_hours: f64,
    pub tasks: Vec<RecoveryTask>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecoveryTask {
    pub id: String,
    pub remaining_hours: f64,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryPlan {
    pub status: String,
    pub planned_remaining_hours: f64,
    pub forecast_remaining_hours: f64,
    pub variance_hours: f64,
    pub delay_hours: f64,
    pub delay_percent: f64,
    pub critical_path_task_ids: Vec<String>,
    pub ready_task_ids: Vec<String>,
    pub blocked_task_ids: Vec<String>,
    pub completed_task_count: usize,
    pub total_task_count: usize,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    EmptyTaskSet,
    TooManyTasks,
    InvalidPlannedHours,
    InvalidTaskId,
    InvalidDependencyId,
    DuplicateTaskId,
    DuplicateDependency,
    InvalidRemainingHours,
    CompletedTaskHasRemainingWork,
    TooManyDependencies,
    UnknownDependency,
    SelfDependency,
    DependencyCycle,
}

pub fn assess(request: RecoveryRequest) -> Result<RecoveryPlan, RecoveryError> {
    if request.tasks.is_empty() {
        return Err(RecoveryError::EmptyTaskSet);
    }
    if request.tasks.len() > MAX_TASKS {
        return Err(RecoveryError::TooManyTasks);
    }
    if !request.planned_remaining_hours.is_finite()
        || request.planned_remaining_hours <= 0.0
        || request.planned_remaining_hours > MAX_ESTIMATE_HOURS
    {
        return Err(RecoveryError::InvalidPlannedHours);
    }

    let mut tasks = BTreeMap::new();
    for task in request.tasks {
        if task.id.is_empty()
            || task.id.len() > 80
            || !task.id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        {
            return Err(RecoveryError::InvalidTaskId);
        }
        if !task.remaining_hours.is_finite()
            || task.remaining_hours < 0.0
            || task.remaining_hours > MAX_ESTIMATE_HOURS
        {
            return Err(RecoveryError::InvalidRemainingHours);
        }
        if task.completed && task.remaining_hours > 0.0 {
            return Err(RecoveryError::CompletedTaskHasRemainingWork);
        }
        if task.depends_on.len() > MAX_DEPENDENCIES_PER_TASK {
            return Err(RecoveryError::TooManyDependencies);
        }
        if tasks.insert(task.id.clone(), task).is_some() {
            return Err(RecoveryError::DuplicateTaskId);
        }
    }

    for task in tasks.values() {
        let mut unique_dependencies = BTreeSet::new();
        for dependency in &task.depends_on {
            if dependency.is_empty()
                || dependency.len() > 80
                || !dependency.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
            {
                return Err(RecoveryError::InvalidDependencyId);
            }
            if dependency == &task.id {
                return Err(RecoveryError::SelfDependency);
            }
            if !tasks.contains_key(dependency) {
                return Err(RecoveryError::UnknownDependency);
            }
            if !unique_dependencies.insert(dependency) {
                return Err(RecoveryError::DuplicateDependency);
            }
        }
    }

    let mut indegree: BTreeMap<String, usize> = tasks
        .iter()
        .map(|(id, task)| (id.clone(), task.depends_on.len()))
        .collect();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, task) in &tasks {
        for dependency in &task.depends_on {
            dependents.entry(dependency.clone()).or_default().push(id.clone());
        }
    }
    for values in dependents.values_mut() {
        values.sort();
    }

    let mut queue: VecDeque<String> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut topological_order = Vec::with_capacity(tasks.len());
    while let Some(id) = queue.pop_front() {
        topological_order.push(id.clone());
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let degree = indegree.get_mut(child).expect("dependent task exists");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    if topological_order.len() != tasks.len() {
        return Err(RecoveryError::DependencyCycle);
    }

    let mut longest_finish: BTreeMap<String, f64> = BTreeMap::new();
    let mut predecessor: BTreeMap<String, Option<String>> = BTreeMap::new();
    for id in &topological_order {
        let task = tasks.get(id).expect("topological task exists");
        let best_dependency = task
            .depends_on
            .iter()
            .filter_map(|dependency| {
                longest_finish
                    .get(dependency)
                    .map(|hours| (dependency, *hours))
            })
            .max_by(|left, right| left.1.total_cmp(&right.1));
        let (prior_id, prior_hours) = best_dependency
            .map(|(dependency, hours)| (Some(dependency.clone()), hours))
            .unwrap_or((None, 0.0));
        let duration = if task.completed { 0.0 } else { task.remaining_hours };
        longest_finish.insert(id.clone(), prior_hours + duration);
        predecessor.insert(id.clone(), prior_id);
    }

    let terminal = longest_finish
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(id, hours)| (id.clone(), *hours));
    let (mut cursor, forecast_remaining_hours) = terminal.unwrap_or_default();
    let mut critical_path_task_ids = Vec::new();
    if forecast_remaining_hours > 0.0 {
        loop {
            if !tasks.get(&cursor).expect("critical task exists").completed {
                critical_path_task_ids.push(cursor.clone());
            }
            match predecessor.get(&cursor).and_then(Clone::clone) {
                Some(previous) => cursor = previous,
                None => break,
            }
        }
        critical_path_task_ids.reverse();
    }

    let ready_task_ids: Vec<String> = tasks
        .iter()
        .filter(|(_, task)| {
            !task.completed
                && task.depends_on.iter().all(|dependency| {
                    tasks.get(dependency).is_some_and(|item| item.completed)
                })
        })
        .map(|(id, _)| id.clone())
        .collect();
    let blocked_task_ids: Vec<String> = tasks
        .iter()
        .filter(|(_, task)| {
            !task.completed
                && task.depends_on.iter().any(|dependency| {
                    tasks.get(dependency).is_some_and(|item| !item.completed)
                })
        })
        .map(|(id, _)| id.clone())
        .collect();

    let variance_hours = forecast_remaining_hours - request.planned_remaining_hours;
    let delay_hours = variance_hours.max(0.0);
    let delay_percent = delay_hours / request.planned_remaining_hours * 100.0;
    let completed_task_count = tasks.values().filter(|task| task.completed).count();
    let status = if completed_task_count == tasks.len() {
        "complete"
    } else if delay_percent <= 5.0 {
        "on_track"
    } else if delay_percent <= 10.0 {
        "optimize"
    } else if delay_percent <= 20.0 {
        "parallelize"
    } else if delay_percent <= 30.0 {
        "rescope"
    } else {
        "replan"
    };

    let recommended_actions = match status {
        "complete" => vec!["Сверить критерии приёмки и закрыть этап только после проверки доказательств".into()],
        "on_track" => vec![
            "Сохранять текущий план и ежедневно обновлять фактические оценки".into(),
            "Не увеличивать число агентов без независимой работы для параллелизации".into(),
        ],
        "optimize" => vec![
            "Перераспределить свободную нагрузку на задачи критического пути".into(),
            "Проверить ожидания ревью, CI и внешних зависимостей".into(),
        ],
        "parallelize" => vec![
            "Разделить независимые задачи на изолированные workspaces и ветки".into(),
            "Назначить отдельного исполнителя и независимого проверяющего".into(),
            "Не допускать параллельной записи в общие файлы и миграции".into(),
        ],
        "rescope" => vec![
            "Отложить необязательные функции, не затрагивая безопасность и критерии выпуска".into(),
            "Разблокировать критический путь и пересмотреть зависимости".into(),
            "Пересчитать сроки после согласования нового объёма".into(),
        ],
        _ => vec![
            "Остановить расширение объёма и построить новый план по фактическим данным".into(),
            "Провести независимый анализ архитектурных, кадровых и внешних блокировок".into(),
            "Сохранить обязательные проверки, безопасность и бюджетные ограничения".into(),
        ],
    };

    Ok(RecoveryPlan {
        status: status.into(),
        planned_remaining_hours: round_hours(request.planned_remaining_hours),
        forecast_remaining_hours: round_hours(forecast_remaining_hours),
        variance_hours: round_hours(variance_hours),
        delay_hours: round_hours(delay_hours),
        delay_percent: round_hours(delay_percent),
        critical_path_task_ids,
        ready_task_ids,
        blocked_task_ids,
        completed_task_count,
        total_task_count: tasks.len(),
        recommended_actions,
    })
}

fn round_hours(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, remaining_hours: f64, depends_on: &[&str]) -> RecoveryTask {
        RecoveryTask {
            id: id.into(),
            remaining_hours,
            completed: false,
            depends_on: depends_on.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn request(planned_remaining_hours: f64, tasks: Vec<RecoveryTask>) -> RecoveryRequest {
        RecoveryRequest { planned_remaining_hours, tasks }
    }

    #[test]
    fn critical_path_uses_longest_dependency_chain_not_total_effort() {
        let plan = assess(request(
            10.0,
            vec![
                task("a", 3.0, &[]),
                task("b", 4.0, &["a"]),
                task("c", 8.0, &[]),
                task("d", 2.0, &["b"]),
            ],
        ))
        .unwrap();

        assert_eq!(plan.forecast_remaining_hours, 9.0);
        assert_eq!(plan.critical_path_task_ids, vec!["a", "b", "d"]);
        assert_eq!(plan.ready_task_ids, vec!["a", "c"]);
        assert_eq!(plan.blocked_task_ids, vec!["b", "d"]);
    }

    #[test]
    fn recovery_thresholds_select_bounded_actions() {
        for (forecast, expected) in [
            (100.0, "on_track"),
            (105.0, "on_track"),
            (108.0, "optimize"),
            (115.0, "parallelize"),
            (125.0, "rescope"),
            (131.0, "replan"),
        ] {
            let plan = assess(request(100.0, vec![task("critical", forecast, &[])])).unwrap();
            assert_eq!(plan.status, expected, "forecast={forecast}");
        }
    }

    #[test]
    fn completed_dependencies_release_downstream_tasks() {
        let mut finished = task("foundation", 0.0, &[]);
        finished.completed = true;
        let plan = assess(request(
            8.0,
            vec![finished, task("feature", 5.0, &["foundation"])],
        ))
        .unwrap();

        assert_eq!(plan.ready_task_ids, vec!["feature"]);
        assert!(plan.blocked_task_ids.is_empty());
        assert_eq!(plan.critical_path_task_ids, vec!["feature"]);
    }

    #[test]
    fn all_completed_tasks_return_complete_without_a_critical_path() {
        let mut first = task("first", 0.0, &[]);
        first.completed = true;
        let mut second = task("second", 0.0, &["first"]);
        second.completed = true;
        let plan = assess(request(4.0, vec![first, second])).unwrap();

        assert_eq!(plan.status, "complete");
        assert!(plan.critical_path_task_ids.is_empty());
        assert_eq!(plan.completed_task_count, 2);
    }

    #[test]
    fn rejects_cycles_unknown_dependencies_and_duplicate_ids() {
        assert_eq!(
            assess(request(4.0, vec![task("a", 1.0, &["b"]), task("b", 1.0, &["a"])]))
                .unwrap_err(),
            RecoveryError::DependencyCycle
        );
        assert_eq!(
            assess(request(4.0, vec![task("a", 1.0, &["missing"])] )).unwrap_err(),
            RecoveryError::UnknownDependency
        );
        assert_eq!(
            assess(request(4.0, vec![task("a", 1.0, &[]), task("a", 2.0, &[])]))
                .unwrap_err(),
            RecoveryError::DuplicateTaskId
        );
    }

    #[test]
    fn rejects_duplicate_dependencies_and_invalid_task_sets() {
        assert_eq!(
            assess(request(4.0, vec![task("a", 1.0, &["base", "base"]), task("base", 1.0, &[])]))
                .unwrap_err(),
            RecoveryError::DuplicateDependency
        );
        assert_eq!(
            assess(request(4.0, vec![])).unwrap_err(),
            RecoveryError::EmptyTaskSet
        );
        assert_eq!(
            assess(request(4.0, vec![task("../unsafe", 1.0, &[])])).unwrap_err(),
            RecoveryError::InvalidTaskId
        );
    }

    #[test]
    fn rejects_unbounded_estimates_and_invalid_dependency_identifiers() {
        assert_eq!(
            assess(request(MAX_ESTIMATE_HOURS + 1.0, vec![task("a", 1.0, &[])]))
                .unwrap_err(),
            RecoveryError::InvalidPlannedHours
        );
        assert_eq!(
            assess(request(4.0, vec![task("a", 1.0, &["../bad"])] )).unwrap_err(),
            RecoveryError::InvalidDependencyId
        );
        assert_eq!(
            assess(request(4.0, vec![task("../bad", 1.0, &[])] )).unwrap_err(),
            RecoveryError::InvalidTaskId
        );
    }

    #[test]
    fn rejects_invalid_effort_and_completed_task_with_remaining_work() {
        assert_eq!(
            assess(request(0.0, vec![task("a", 1.0, &[])])).unwrap_err(),
            RecoveryError::InvalidPlannedHours
        );
        assert_eq!(
            assess(request(4.0, vec![task("a", f64::NAN, &[])])).unwrap_err(),
            RecoveryError::InvalidRemainingHours
        );
        let mut inconsistent = task("a", 1.0, &[]);
        inconsistent.completed = true;
        assert_eq!(
            assess(request(4.0, vec![inconsistent])).unwrap_err(),
            RecoveryError::CompletedTaskHasRemainingWork
        );
    }
}
