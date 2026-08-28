use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFieldV1 {
    pub name: String,
    pub value: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskCandidatePayloadV1 {
    pub title: String,
    pub start_at: Option<String>,
    pub due_at: Option<String>,
    pub due_expression: Option<String>,
    pub location: Option<String>,
    pub submission_url: Option<String>,
    pub materials: Vec<String>,
    pub audience: Option<String>,
    pub required: Option<bool>,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResultV1 {
    pub schema_version: u16,
    pub revision_id: String,
    pub classifier_version: String,
    pub normalized_text: String,
    pub category: String,
    pub category_confidence: f32,
    pub fields: Vec<ExtractedFieldV1>,
    pub candidates: Vec<TaskCandidatePayloadV1>,
    pub warnings: Vec<String>,
    pub requires_review: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnderstandingError {
    TextRequired,
    InvalidModelOutput,
    ModelTimeout,
    ModelCancelled,
    ModelCrashed,
    DateInvalid,
}

impl std::fmt::Display for UnderstandingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::TextRequired => "ANALYSIS_TEXT_REQUIRED",
            Self::InvalidModelOutput => "ANALYSIS_MODEL_INVALID_JSON",
            Self::ModelTimeout => "ANALYSIS_MODEL_TIMEOUT",
            Self::ModelCancelled => "ANALYSIS_CANCELLED",
            Self::ModelCrashed => "ANALYSIS_MODEL_CRASHED",
            Self::DateInvalid => "ANALYSIS_DATE_INVALID",
        };
        f.write_str(code)
    }
}

impl std::error::Error for UnderstandingError {}

pub const ANALYSIS_SCHEMA_VERSION: u16 = 1;
pub const MODEL_PROMPT_VERSION: &str = "campus-notice-semantic-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructuredModelTaskV1 {
    pub title: String,
    pub time_expression: Option<String>,
    pub location_or_entry: Option<String>,
    pub materials: Vec<String>,
    pub audience: Option<String>,
    pub required: Option<bool>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructuredModelOutputV1 {
    pub category: String,
    pub change_intent: String,
    pub tasks: Vec<StructuredModelTaskV1>,
    pub uncertainties: Vec<String>,
}

pub fn build_model_prompt(text: &str, published_at: &str, timezone: &str) -> String {
    format!(
        "提示词版本: {MODEL_PROMPT_VERSION}\n通知发布时间: {published_at}\n时区: {timezone}\n通知原文:\n{text}"
    )
}

pub fn parse_structured_model_output(
    raw: &str,
) -> Result<StructuredModelOutputV1, UnderstandingError> {
    let output: StructuredModelOutputV1 =
        serde_json::from_str(raw).map_err(|_| UnderstandingError::InvalidModelOutput)?;
    if !matches!(
        output.category.as_str(),
        "required-action" | "schedule" | "voluntary" | "result-or-change" | "information-only"
    ) || !matches!(
        output.change_intent.as_str(),
        "none" | "reschedule" | "cancel"
    ) || output.tasks.len() > 5
        || output
            .tasks
            .iter()
            .any(|task| task.title.trim().is_empty() || task.evidence.is_empty())
    {
        return Err(UnderstandingError::InvalidModelOutput);
    }
    Ok(output)
}

pub fn normalize_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        let mapped = match character {
            '\u{3000}' => ' ',
            '：' => ':',
            '，' => ',',
            '。' => '.',
            '；' => ';',
            '！' => '!',
            '？' => '?',
            '（' => '(',
            '）' => ')',
            '【' => '[',
            '】' => ']',
            '“' | '”' => '"',
            '‘' | '’' => '\'',
            '\r' => '\n',
            other => other,
        };
        output.push(mapped);
    }
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_model_output(raw: &str) -> Result<serde_json::Value, UnderstandingError> {
    if raw.contains("__TIMEOUT__") {
        return Err(UnderstandingError::ModelTimeout);
    }
    if raw.contains("__CANCELLED__") {
        return Err(UnderstandingError::ModelCancelled);
    }
    if raw.contains("__CRASH__") {
        return Err(UnderstandingError::ModelCrashed);
    }
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| UnderstandingError::InvalidModelOutput)?;
    if !value.is_object()
        || value
            .get("category")
            .and_then(|value| value.as_str())
            .is_none()
    {
        return Err(UnderstandingError::InvalidModelOutput);
    }
    Ok(value)
}

pub fn analyze_text(
    text: &str,
    published_at: &str,
    revision_id: String,
) -> Result<AnalysisResultV1, UnderstandingError> {
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return Err(UnderstandingError::TextRequired);
    }
    let rules = extract_rules(&normalized, published_at)?;
    let category = classify(&normalized);
    let semantic_json = serde_json::json!({ "category": category });
    let _semantic_output = parse_model_output(&semantic_json.to_string())?;
    let mut warnings = rules.warnings;
    warnings.push("本地千问不可用或输出未通过校验，已使用规则回退；结果可信度较低。".to_owned());
    let mut candidates = rules.candidates;
    if category == "informationOnly" {
        candidates.clear();
    }
    let category_confidence = if category == "pendingReview" {
        0.35
    } else {
        0.58
    };
    let requires_review = category_confidence < 0.75
        || candidates
            .iter()
            .any(|candidate| candidate.status != "trusted");
    if candidates.is_empty() && category != "informationOnly" {
        warnings.push("未找到明确的可执行动作，请人工补充或标记为仅供知晓".to_owned());
    }
    Ok(AnalysisResultV1 {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        revision_id,
        classifier_version: "rules-semantic-fallback-v1".to_owned(),
        normalized_text: normalized,
        category,
        category_confidence,
        fields: rules.fields,
        candidates,
        warnings,
        requires_review,
    })
}

pub fn analyze_text_with_model_output(
    text: &str,
    published_at: &str,
    revision_id: String,
    raw_model_output: &str,
) -> Result<AnalysisResultV1, UnderstandingError> {
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return Err(UnderstandingError::TextRequired);
    }
    let model = parse_structured_model_output(raw_model_output)?;
    let rules = extract_rules(&normalized, published_at)?;
    let category = model_category(&model.category);
    let mut warnings = model.uncertainties.clone();
    let mut candidates = Vec::new();
    for task in model.tasks {
        if task
            .evidence
            .iter()
            .any(|evidence| !normalized.contains(evidence))
        {
            return Err(UnderstandingError::InvalidModelOutput);
        }
        let expression = task.time_expression.clone();
        let due_at = expression.as_deref().and_then(|value| {
            resolve_relative(value, published_at)
                .ok()
                .or_else(|| resolve_absolute(value, published_at).ok())
                .map(|resolved| apply_time(resolved, find_time(value).as_deref()))
        });
        let status = if due_at.is_none() && expression.is_some() {
            warnings.push(format!("任务“{}”的时间表达需要核对", task.title));
            "needsReview"
        } else if task.required.is_none() || task.audience.is_none() {
            "missing"
        } else {
            "trusted"
        };
        candidates.push(TaskCandidatePayloadV1 {
            title: task.title,
            start_at: None,
            due_at,
            due_expression: expression,
            location: task.location_or_entry,
            submission_url: rules
                .fields
                .iter()
                .find(|field| field.name == "submissionUrl")
                .and_then(|field| field.value.clone()),
            materials: task.materials,
            audience: task.audience,
            required: task.required,
            confidence: if status == "trusted" { 0.9 } else { 0.55 },
            evidence: task.evidence,
            status: status.to_owned(),
        });
    }
    let requires_review = !warnings.is_empty()
        || candidates
            .iter()
            .any(|candidate| candidate.status != "trusted")
        || model.change_intent != "none";
    Ok(AnalysisResultV1 {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        revision_id,
        classifier_version: format!("qwen-{}-v{}", MODEL_PROMPT_VERSION, ANALYSIS_SCHEMA_VERSION),
        normalized_text: normalized,
        category,
        category_confidence: if requires_review { 0.7 } else { 0.92 },
        fields: rules.fields,
        candidates,
        warnings,
        requires_review,
    })
}

fn model_category(category: &str) -> String {
    match category {
        "required-action" => "mustComplete",
        "schedule" => "schedule",
        "voluntary" => "optional",
        "result-or-change" => "resultOrChange",
        _ => "informationOnly",
    }
    .to_owned()
}

struct RuleExtraction {
    fields: Vec<ExtractedFieldV1>,
    candidates: Vec<TaskCandidatePayloadV1>,
    warnings: Vec<String>,
}

fn extract_rules(text: &str, published_at: &str) -> Result<RuleExtraction, UnderstandingError> {
    let mut fields = Vec::new();
    let mut warnings = Vec::new();
    let url = find_url(text);
    fields.push(field(
        "submissionUrl",
        url.clone(),
        url.iter().cloned().collect(),
        0.96,
    ));
    let phone = find_phone(text);
    fields.push(field(
        "contactPhone",
        phone.clone(),
        phone.iter().cloned().collect(),
        0.9,
    ));
    let location = find_location(text);
    fields.push(field(
        "location",
        location.clone(),
        location.iter().cloned().collect(),
        0.82,
    ));
    let materials = find_materials(text);
    fields.push(field(
        "materials",
        (!materials.is_empty()).then(|| materials.join(", ")),
        materials.clone(),
        0.84,
    ));
    let audience = find_audience(text);
    fields.push(field(
        "audience",
        audience.clone(),
        audience.iter().cloned().collect(),
        0.78,
    ));
    let required = find_required(text);
    fields.push(field(
        "required",
        required.map(|value| value.to_string()),
        Vec::new(),
        if required.is_some() { 0.9 } else { 0.45 },
    ));

    let expressions = find_date_expressions(text);
    let time = find_time(text);
    fields.push(field(
        "time",
        time.clone(),
        time.iter().cloned().collect(),
        if time.is_some() { 0.9 } else { 0.45 },
    ));
    let distinct_dates = expressions
        .iter()
        .filter_map(|(_, expression)| absolute_date(expression))
        .collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for (index, line) in action_lines(text).into_iter().enumerate() {
        let expression = relative_expression(line).or_else(|| {
            find_date_expressions(line)
                .into_iter()
                .next()
                .map(|(_, value)| value)
        });
        let parsed = expression
            .as_deref()
            .and_then(|value| {
                resolve_relative(value, published_at)
                    .ok()
                    .or_else(|| resolve_absolute(value, published_at).ok())
            })
            .map(|value| apply_time(value, find_time(line).or_else(|| time.clone()).as_deref()));
        let has_conflict = distinct_dates.len() > 1 && expression.is_some();
        let status = if has_conflict {
            "conflict"
        } else if parsed.is_some() || expression.is_none() {
            "trusted"
        } else {
            "needsReview"
        };
        if parsed.is_none() && !expressions.is_empty() {
            warnings.push(format!("第 {} 项任务的时间表达需要核对", index + 1));
        }
        if has_conflict {
            warnings.push(format!(
                "第 {} 项任务存在多个冲突时间表达，请人工选择",
                index + 1
            ));
        }
        candidates.push(TaskCandidatePayloadV1 {
            title: action_title(line),
            start_at: None,
            due_at: parsed,
            due_expression: expression,
            location: location.clone(),
            submission_url: url.clone(),
            materials: materials.clone(),
            audience: audience.clone(),
            required,
            confidence: if status == "trusted" { 0.84 } else { 0.42 },
            evidence: vec![line.to_owned()],
            status: status.to_owned(),
        });
    }
    if distinct_dates.len() > 1 {
        warnings.push("检测到多个时间表达，请确认各任务对应的截止时间".to_owned());
    }
    Ok(RuleExtraction {
        fields,
        candidates,
        warnings,
    })
}

fn field(
    name: &str,
    value: Option<String>,
    evidence: Vec<String>,
    confidence: f32,
) -> ExtractedFieldV1 {
    ExtractedFieldV1 {
        name: name.to_owned(),
        status: if value.is_some() {
            "trusted"
        } else {
            "missing"
        }
        .to_owned(),
        value,
        confidence,
        evidence,
    }
}

fn classify(text: &str) -> String {
    if text.contains("取消")
        || text.contains("改期")
        || text.contains("调整")
        || text.contains("变更")
    {
        return "resultOrChange".to_owned();
    }
    if text.contains("提交")
        || text.contains("报名")
        || text.contains("确认")
        || text.contains("缴费")
        || text.contains("必须")
        || text.contains("务必")
    {
        return "mustComplete".to_owned();
    }
    if text.contains("时间")
        || text.contains("地点")
        || text.contains("会议")
        || text.contains("活动")
    {
        return "schedule".to_owned();
    }
    if text.contains("欢迎") || text.contains("自愿") || text.contains("可报名") {
        return "optional".to_owned();
    }
    "informationOnly".to_owned()
}

fn action_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let action = [
                "提交", "报名", "确认", "缴费", "参加", "填写", "领取", "上传", "预约",
            ]
            .iter()
            .any(|keyword| trimmed.contains(keyword));
            action.then_some(trimmed)
        })
        .collect()
}

fn action_title(line: &str) -> String {
    let mut title = line.trim().to_owned();
    for marker in ["截止", "前", "，", ",", ":", "："] {
        if let Some(index) = title.find(marker) {
            title.truncate(index);
        }
    }
    title.trim().trim_matches(['-', '·', ' ']).to_owned()
}

fn find_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| {
            let bytes = token.as_bytes();
            let separator = *b"://";
            (bytes.starts_with(b"http") && bytes.get(4..7) == Some(separator.as_slice()))
                || (bytes.starts_with(b"https") && bytes.get(5..8) == Some(separator.as_slice()))
        })
        .map(|token| {
            token
                .trim_matches(['，', ',', '。', '.', ')', '）'])
                .to_owned()
        })
}

fn find_phone(text: &str) -> Option<String> {
    text.split(|character: char| !character.is_ascii_digit())
        .find(|token| token.len() == 11 && token.starts_with('1'))
        .map(str::to_owned)
}

fn find_time(text: &str) -> Option<String> {
    for token in text.split(|character: char| !character.is_ascii_digit() && character != ':') {
        let Some((hour, minute)) = token.split_once(':') else {
            continue;
        };
        let (Ok(hour), Ok(minute)) = (hour.parse::<u32>(), minute.parse::<u32>()) else {
            continue;
        };
        if hour < 24 && minute < 60 {
            return Some(format!("{hour:02}:{minute:02}"));
        }
    }
    for value in text.split('点').take(text.matches('点').count()) {
        let digits = value
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        let Ok(hour) = digits.chars().rev().collect::<String>().parse::<u32>() else {
            continue;
        };
        if hour < 24 {
            return Some(format!("{hour:02}:00"));
        }
    }
    None
}

fn apply_time(date_time: String, time: Option<&str>) -> String {
    let Some(time) = time else {
        return date_time;
    };
    format!("{}T{time}:00Z", &date_time[..10])
}

fn find_location(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| {
            ["教室", "会议室", "校区", "地点", "礼堂", "体育馆"]
                .iter()
                .any(|word| line.contains(word))
        })
        .map(str::to_owned)
}

fn find_materials(text: &str) -> Vec<String> {
    let mut values = BTreeSet::new();
    for keyword in [
        "身份证",
        "学生证",
        "报名表",
        "申请表",
        "照片",
        "材料",
        "证明",
        "缴费凭证",
        "附件",
    ] {
        if text.contains(keyword) {
            values.insert(keyword.to_owned());
        }
    }
    values.into_iter().collect()
}

fn find_audience(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| {
            [
                "全体",
                "本科生",
                "研究生",
                "大一",
                "大二",
                "大三",
                "大四",
                "班级",
            ]
            .iter()
            .any(|word| line.contains(word))
        })
        .map(str::to_owned)
}

fn find_required(text: &str) -> Option<bool> {
    if ["必须", "务必", "请于", "截止"]
        .iter()
        .any(|word| text.contains(word))
    {
        Some(true)
    } else if ["自愿", "可选", "欢迎参加"]
        .iter()
        .any(|word| text.contains(word))
    {
        Some(false)
    } else {
        None
    }
}

fn find_date_expressions(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .flat_map(|(index, line)| {
            let relative = [
                "今天",
                "明天",
                "后天",
                "本周一",
                "本周二",
                "本周三",
                "本周四",
                "本周五",
                "本周六",
                "本周日",
                "下周一",
                "下周二",
                "下周三",
                "下周四",
                "下周五",
                "下周六",
                "下周日",
                "本周",
                "下周",
                "月底",
                "近期",
            ];
            let mut values = relative
                .iter()
                .filter(|word| line.contains(**word))
                .map(|word| (index, (*word).to_owned()))
                .collect::<Vec<_>>();
            values.extend(
                find_absolute_dates(line)
                    .into_iter()
                    .map(|value| (index, value)),
            );
            values
        })
        .collect()
}

fn relative_expression(line: &str) -> Option<String> {
    [
        "今天",
        "明天",
        "后天",
        "本周一",
        "本周二",
        "本周三",
        "本周四",
        "本周五",
        "本周六",
        "本周日",
        "下周一",
        "下周二",
        "下周三",
        "下周四",
        "下周五",
        "下周六",
        "下周日",
        "本周",
        "下周",
        "月底",
        "近期",
    ]
    .iter()
    .find(|word| line.contains(**word))
    .map(|word| (*word).to_owned())
}

fn resolve_relative(expression: &str, published_at: &str) -> Result<String, UnderstandingError> {
    let date = parse_iso_date(published_at)?;
    let days = match expression {
        "今天" => 0,
        "明天" => 1,
        "后天" => 2,
        "本周一" => weekday_offset(date, 1, false),
        "本周二" => weekday_offset(date, 2, false),
        "本周三" => weekday_offset(date, 3, false),
        "本周四" => weekday_offset(date, 4, false),
        "本周五" => weekday_offset(date, 5, false),
        "本周六" => weekday_offset(date, 6, false),
        "本周日" => weekday_offset(date, 7, false),
        "下周一" => weekday_offset(date, 1, true),
        "下周二" => weekday_offset(date, 2, true),
        "下周三" => weekday_offset(date, 3, true),
        "下周四" => weekday_offset(date, 4, true),
        "下周五" => weekday_offset(date, 5, true),
        "下周六" => weekday_offset(date, 6, true),
        "下周日" => weekday_offset(date, 7, true),
        "本周" | "下周" | "月底" | "近期" => return Err(UnderstandingError::DateInvalid),
        _ => return Err(UnderstandingError::DateInvalid),
    };
    let (year, month, day) = add_days(date, days);
    Ok(format!("{year:04}-{month:02}-{day:02}T23:59:00Z"))
}

fn weekday_offset(date: (i32, u32, u32), weekday: i32, next_week: bool) -> i32 {
    let current = weekday_of(date);
    let week_start_offset = 1 - current;
    week_start_offset + weekday - 1 + if next_week { 7 } else { 0 }
}

fn weekday_of((year, month, day): (i32, u32, u32)) -> i32 {
    let (mut y, mut m) = (year, month as i32);
    if m < 3 {
        y -= 1;
        m += 12;
    }
    let k = y % 100;
    let j = y / 100;
    let h = (day as i32 + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    match h {
        0 => 6,
        1 => 7,
        2 => 1,
        3 => 2,
        4 => 3,
        5 => 4,
        _ => 5,
    }
}

fn find_absolute_dates(line: &str) -> Vec<String> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut values = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        if !chars[start].is_ascii_digit() {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < chars.len()
            && (chars[end].is_ascii_digit() || matches!(chars[end], '-' | '/' | '年' | '月' | '日'))
        {
            end += 1;
        }
        let candidate = chars[start..end].iter().collect::<String>();
        if let Some((year, month, day)) = parse_date_expression(&candidate) {
            values.push(format!("{year:04}-{month:02}-{day:02}"));
        }
        start = end;
    }
    values
}

fn parse_date_expression(value: &str) -> Option<(i32, u32, u32)> {
    let normalized = value.replace(['年', '月'], "-").replace('日', "");
    let parts = normalized
        .split(['-', '/'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let (year, month, day) = match parts.as_slice() {
        [year, month, day] if year.len() == 4 => {
            (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
        }
        [month, day] => (0, month.parse().ok()?, day.parse().ok()?),
        _ => return None,
    };
    if !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year.max(2024), month as i32) as u32
    {
        return None;
    }
    Some((year, month, day))
}

fn absolute_date(expression: &str) -> Option<String> {
    let (year, month, day) = parse_date_expression(expression)?;
    (year > 0).then(|| format!("{year:04}-{month:02}-{day:02}"))
}

fn resolve_absolute(expression: &str, published_at: &str) -> Result<String, UnderstandingError> {
    let (mut year, month, day) =
        parse_date_expression(expression).ok_or(UnderstandingError::DateInvalid)?;
    if year == 0 {
        year = parse_iso_date(published_at)?.0;
    }
    if day > days_in_month(year, month as i32) as u32 {
        return Err(UnderstandingError::DateInvalid);
    }
    Ok(format!("{year:04}-{month:02}-{day:02}T23:59:00Z"))
}

fn parse_iso_date(value: &str) -> Result<(i32, u32, u32), UnderstandingError> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(UnderstandingError::DateInvalid);
    }
    let year = value[0..4]
        .parse()
        .map_err(|_| UnderstandingError::DateInvalid)?;
    let month = value[5..7]
        .parse()
        .map_err(|_| UnderstandingError::DateInvalid)?;
    let day = value[8..10]
        .parse()
        .map_err(|_| UnderstandingError::DateInvalid)?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month as i32) as u32 {
        return Err(UnderstandingError::DateInvalid);
    }
    Ok((year, month, day))
}

fn add_days((year, month, day): (i32, u32, u32), days: i32) -> (i32, u32, u32) {
    let mut ordinal = day as i32 + days;
    let mut y = year;
    let mut m = month as i32;
    loop {
        let limit = days_in_month(y, m);
        if ordinal <= limit {
            break;
        }
        ordinal -= limit;
        m += 1;
        if m > 12 {
            y += 1;
            m = 1;
        }
    }
    while ordinal < 1 {
        m -= 1;
        if m < 1 {
            y -= 1;
            m = 12;
        }
        ordinal += days_in_month(y, m);
    }
    (y, m as u32, ordinal as u32)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_text, analyze_text_with_model_output, normalize_text, parse_model_output,
        parse_structured_model_output, resolve_absolute, resolve_relative, UnderstandingError,
    };

    #[test]
    fn normalizes_full_width_punctuation_and_blank_lines() {
        assert_eq!(
            normalize_text("  提交：材料　\n\n截止：明天"),
            "提交:材料\n截止:明天"
        );
    }

    #[test]
    fn rules_extract_multiple_actions_and_relative_due_date() {
        let result = analyze_text(
            "请务必完成以下事项\n提交报名表，明天前\n上传身份证，明天前\n地点：信息楼教室",
            "2026-08-27T09:00:00Z",
            "revision-1".to_owned(),
        )
        .unwrap();
        assert_eq!(result.category, "mustComplete");
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(
            result.candidates[0].due_at.as_deref(),
            Some("2026-08-28T23:59:00Z")
        );
        assert_eq!(
            result
                .fields
                .iter()
                .find(|field| field.name == "location")
                .unwrap()
                .value
                .as_deref(),
            Some("地点:信息楼教室")
        );
    }

    #[test]
    fn model_adapter_rejects_invalid_and_recovers_controlled_failures() {
        assert!(matches!(
            parse_model_output("{}"),
            Err(UnderstandingError::InvalidModelOutput)
        ));
        assert!(matches!(
            parse_model_output("__TIMEOUT__"),
            Err(UnderstandingError::ModelTimeout)
        ));
        assert!(parse_model_output(r#"{"category":"mustComplete"}"#).is_ok());
    }

    #[test]
    fn structured_model_contract_rejects_unknown_fields_and_missing_evidence() {
        let valid = r#"{"category":"required-action","changeIntent":"none","tasks":[{"title":"提交报名","timeExpression":null,"locationOrEntry":null,"materials":[],"audience":null,"required":true,"evidence":["提交报名"]}],"uncertainties":[]}"#;
        assert!(parse_structured_model_output(valid).is_ok());
        assert!(parse_structured_model_output(
            r#"{"category":"required-action","changeIntent":"none","tasks":[],"uncertainties":[],"unexpected":true}"#
        )
        .is_err());
        assert!(parse_structured_model_output(
            r#"{"category":"required-action","changeIntent":"none","tasks":[{"title":"提交报名","timeExpression":null,"locationOrEntry":null,"materials":[],"audience":null,"required":true,"evidence":[]}],"uncertainties":[]}"#
        )
        .is_err());
    }

    #[test]
    fn structured_model_output_is_mapped_to_independent_candidates_with_resolved_dates() {
        let raw = r#"{"category":"required-action","changeIntent":"none","tasks":[{"title":"提交报名","timeExpression":"明天前","locationOrEntry":"线上表单","materials":["报名表"],"audience":"全体学生","required":true,"evidence":["请提交报名"]},{"title":"参加说明会","timeExpression":null,"locationOrEntry":"报告厅","materials":[],"audience":"报名学生","required":true,"evidence":["参加说明会"]}],"uncertainties":[]}"#;
        let result = analyze_text_with_model_output(
            "请提交报名，参加说明会",
            "2026-08-28T09:00:00Z",
            "revision-model".to_owned(),
            raw,
        )
        .unwrap();
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(
            result.candidates[0].due_at.as_deref(),
            Some("2026-08-29T23:59:00Z")
        );
        assert_eq!(result.candidates[1].title, "参加说明会");
    }

    #[test]
    fn model_evidence_must_be_present_in_original_text() {
        let raw = r#"{"category":"required-action","changeIntent":"none","tasks":[{"title":"提交报名","timeExpression":null,"locationOrEntry":null,"materials":[],"audience":null,"required":true,"evidence":["原文没有这句话"]}],"uncertainties":[]}"#;
        assert!(matches!(
            analyze_text_with_model_output(
                "请提交报名",
                "2026-08-28T09:00:00Z",
                "revision-invalid".to_owned(),
                raw,
            ),
            Err(UnderstandingError::InvalidModelOutput)
        ));
    }

    #[test]
    fn resolves_absolute_dates_and_rejects_invalid_calendar_days() {
        assert_eq!(
            resolve_absolute("2026年8月28日", "2026-08-01T09:00:00Z").unwrap(),
            "2026-08-28T23:59:00Z"
        );
        assert_eq!(
            resolve_absolute("8/28", "2026-08-01T09:00:00Z").unwrap(),
            "2026-08-28T23:59:00Z"
        );
        assert!(resolve_absolute("2026-02-30", "2026-08-01T09:00:00Z").is_err());
    }

    #[test]
    fn resolves_this_and_next_week_without_silently_rolling_this_week_forward() {
        let published_at = "2026-08-28T09:00:00Z";
        assert_eq!(
            resolve_relative("本周一", published_at).unwrap(),
            "2026-08-24T23:59:00Z"
        );
        assert_eq!(
            resolve_relative("下周一", published_at).unwrap(),
            "2026-08-31T23:59:00Z"
        );
        assert!(resolve_relative("下周", published_at).is_err());
    }

    #[test]
    fn marks_multiple_absolute_due_dates_as_a_conflict() {
        let result = analyze_text(
            "请提交报名表，截止2026-08-28；如有补充材料，截止2026-08-30。",
            "2026-08-01T09:00:00Z",
            "revision-conflict".to_owned(),
        )
        .unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].status, "conflict");
        assert!(result.requires_review);
    }

    #[test]
    fn extracts_clock_time_with_a_date_expression() {
        let result = analyze_text(
            "请于8月28日 17:30前提交报名表。",
            "2026-08-01T09:00:00Z",
            "revision-time".to_owned(),
        )
        .unwrap();
        assert_eq!(
            result.candidates[0].due_at.as_deref(),
            Some("2026-08-28T17:30:00Z")
        );
        assert_eq!(
            result
                .fields
                .iter()
                .find(|field| field.name == "time")
                .and_then(|field| field.value.as_deref()),
            Some("17:30")
        );
    }
}
