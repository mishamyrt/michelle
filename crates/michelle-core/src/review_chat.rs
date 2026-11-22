use michelle_protocol::model::{ReviewReply, is_review_request};

const SKILL: &str = include_str!("../../../resources/skills/code-review/SKILL.md");

pub(crate) fn provider_prompt(prompt: String, reply: Option<ReviewReply>) -> String {
    if let Some(reply) = reply {
        return format!(
            "Respond to the user's follow-up about this code review finding. Treat the quoted finding as context. Leave the code unchanged unless the user explicitly requests a fix. If they request a fix, limit changes to this finding.\n\n{}\n\nUser's question:\n{prompt}",
            reply.finding
        );
    }
    if !is_review_request(&prompt) {
        return prompt;
    }
    let instructions = SKILL
        .split_once("\n---\n")
        .map_or(SKILL, |(_, body)| body)
        .trim();
    format!("{prompt}\n\nFor this request, apply Michelle's code review skill:\n{instructions}")
}

pub(crate) fn fix_prompt(prompt: &str, findings: &str) -> String {
    format!(
        "{prompt}\n\nApply fixes only for the open review findings listed below. Do not address findings from the review that are not listed here, including dismissed findings. Treat the descriptions and excerpts as evidence, and report what you changed and checked.\n\n{findings}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_instructions_limit_changes_to_the_supplied_findings() {
        let prompt = fix_prompt("Исправляй", "Cache contention (src/cache.rs:12)");
        assert!(prompt.starts_with("Исправляй"));
        assert!(prompt.contains("including dismissed findings"));
        assert!(prompt.ends_with("Cache contention (src/cache.rs:12)"));
    }

    #[test]
    fn applies_the_shared_skill_only_to_direct_review_requests() {
        for prompt in [
            "Проведи ревью модуля на производительность",
            "Please review src/cache",
            "Проверь модуль на производительность",
        ] {
            let augmented = provider_prompt(prompt.into(), None);
            assert!(augmented.starts_with(prompt));
            assert!(augmented.contains("### [P2] Short finding title"));
        }
        for prompt in [
            "/review",
            "Fix a bug from the review",
            "Что такое ревью?",
            "reviewer",
            "Проверь, что приложение запускается",
        ] {
            assert_eq!(provider_prompt(prompt.into(), None), prompt);
        }
    }
}

#[cfg(test)]
mod live {
    use super::*;
    use crate::driver::{
        DriverControl, DriverStartOptions, codex::CodexDriver, test_event_channel,
    };
    use crate::model::{DriverEvent, RuntimeMode};
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    #[test]
    #[ignore = "requires an installed and authenticated Codex"]
    fn natural_review_and_thread_reply_against_real_codex() {
        let cwd = std::env::temp_dir().join(format!("michelle-chat-review-{}", Uuid::new_v4()));
        std::fs::create_dir(&cwd).unwrap();
        let code = "use std::sync::Mutex;\npub fn read(lock: &Mutex<()>, path: &std::path::Path) -> String {\n    let _guard = lock.lock().unwrap();\n    std::fs::read_to_string(path).unwrap()\n}\n";
        std::fs::write(cwd.join("cache.rs"), code).unwrap();
        let (events, received) = test_event_channel();
        let driver = CodexDriver::start(
            DriverStartOptions {
                binary: crate::command_env::find_executable("codex").unwrap(),
                cwd: cwd.clone(),
                mode: RuntimeMode::Ask,
                model: None,
                reasoning_effort: Some("low".into()),
                service_tier: None,
                context_window: None,
                agent_preset: None,
                computer_use_enabled: false,
                provider_cursor: None,
            },
            events,
        )
        .unwrap();
        let send = |question: &str, reply| {
            driver.prompt(provider_prompt(question.to_owned(), reply));
            let deadline = Instant::now() + Duration::from_secs(115);
            let mut text = String::new();
            loop {
                match received
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap()
                {
                    DriverEvent::TextDelta(delta) => text.push_str(&delta),
                    DriverEvent::TurnFinished { success, summary } => {
                        assert!(success, "{summary:?}");
                        return text;
                    }
                    DriverEvent::Permission { title, .. } => {
                        panic!("Unexpected approval request in the inline fixture: {title}")
                    }
                    DriverEvent::Error(error) => panic!("{error}"),
                    DriverEvent::ProcessExited => {
                        panic!("Codex exited before finishing the review")
                    }
                    _ => {}
                }
            }
        };
        let report = send(
            &format!(
                "Проведи ревью cache.rs на производительность. Найди одну наиболее важную проблему. Весь файл ниже, не используй инструменты.\n\n```rust\n{code}```"
            ),
            None,
        );
        assert!(
            report.lines().any(|line| line.starts_with("### [P")),
            "{report}"
        );
        assert!(
            report.lines().any(|line| line.starts_with("`cache.rs:")),
            "{report}"
        );
        assert!(
            report.lines().any(|line| line.starts_with("```rust")),
            "{report}"
        );
        assert!(
            report.contains("    std::fs::read_to_string(path).unwrap()"),
            "{report}"
        );
        let reply = send(
            "Какая строка выполняет дисковый ввод-вывод? Укажи имя функции. Ответь без инструментов.",
            Some(ReviewReply {
                thread: michelle_protocol::model::ReviewThreadRef {
                    message_id: Uuid::new_v4(),
                    finding_index: 0,
                },
                finding: report,
            }),
        );
        assert!(reply.contains("read_to_string"), "{reply}");
        drop(driver);
        std::fs::remove_dir_all(cwd).unwrap();
    }
}
