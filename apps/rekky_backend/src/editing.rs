//! Explicit owner-authored replacements. No model call or source rewrite.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditInput {
    pub subject: String,
    pub visibility: String,
    pub entity_kind: String,
    pub summary: String,
    pub experience: String,
    pub attribution: String,
    pub observations: Vec<Observation>,
    pub locations: Vec<Location>,
    pub use_cases: Vec<String>,
    pub types: Vec<String>,
    pub facets: Vec<String>,
    pub descriptors: Vec<String>,
    pub destination: Destination,
    #[serde(default)]
    pub destination_confirmed: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub kind: String,
    pub text: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub role: String,
    pub text: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub mode: String,
    pub url: String,
    pub label: String,
}

fn clean(value: &mut String, min: usize, max: usize) -> Result<(), &'static str> {
    *value = value.trim().to_owned();
    if !(min..=max).contains(&value.chars().count()) || value.contains('\0') {
        return Err("A field is empty or too long");
    }
    Ok(())
}
impl EditInput {
    pub fn normalized(mut self) -> Result<Self, &'static str> {
        clean(&mut self.subject, 1, 120)?;
        clean(&mut self.summary, 1, 20000)?;
        clean(&mut self.attribution, 0, 300)?;
        if !["private", "friends"].contains(&self.visibility.as_str()) {
            return Err("Choose Only me or Friends");
        }
        if !["firsthand", "secondhand", "interest", "unspecified"]
            .contains(&self.experience.as_str())
        {
            return Err("Choose a valid experience");
        }
        self.shelf()?;
        if self.observations.len() > 40
            || self.locations.len() > 12
            || self.use_cases.len() > 20
            || self.descriptors.len() > 4
        {
            return Err("Too many details");
        }
        for o in &mut self.observations {
            if ![
                "praise",
                "suggestion",
                "suitability",
                "caution",
                "price",
                "context",
            ]
            .contains(&o.kind.as_str())
            {
                return Err("Unknown detail type");
            }
            clean(&mut o.text, 1, 2000)?;
        }
        for l in &mut self.locations {
            if ![
                "venue",
                "practice",
                "service_area",
                "past_experience",
                "context",
            ]
            .contains(&l.role.as_str())
            {
                return Err("Unknown location type");
            }
            clean(&mut l.text, 1, 300)?;
        }
        for text in &mut self.use_cases {
            clean(text, 1, 200)?;
        }
        for text in &mut self.descriptors {
            clean(text, 1, 100)?;
        }
        clean(&mut self.destination.url, 0, 2048)?;
        clean(&mut self.destination.label, 0, 40)?;
        match self.destination.mode.as_str() {
            "auto" | "none" => {
                self.destination.url.clear();
                self.destination.label.clear();
            }
            "custom" => {
                let url = reqwest::Url::parse(&self.destination.url)
                    .map_err(|_| "Enter a valid HTTPS link")?;
                if url.scheme() != "https"
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || self.destination.url.chars().any(char::is_control)
                {
                    return Err("Use an HTTPS link without embedded credentials");
                }
                if self.destination.label.is_empty() {
                    self.destination.label = "Open link".into();
                }
            }
            _ => return Err("Choose a valid link option"),
        }
        Ok(self)
    }
    fn shelf(&self) -> Result<&'static str, &'static str> {
        match self.entity_kind.as_str() {
            "place" => Ok("Places"),
            "person_service" => Ok("People & services"),
            "thing" => Ok("Things"),
            "activity_event" => Ok("Activities & events"),
            "idea_tip" => Ok("Ideas & tips"),
            _ => Err("Choose a valid recommendation kind"),
        }
    }
    pub fn build(&self) -> Result<(Value, String), &'static str> {
        let classification = crate::taxonomy::present(
            &self.entity_kind,
            &self.types,
            &self.facets,
            &self.descriptors,
            "user",
        )
        .ok_or("Unknown, duplicate or incompatible category")?;
        let mut body = vec![self.summary.clone(), self.attribution.clone()];
        body.extend(self.observations.iter().map(|o| o.text.clone()));
        body.extend(self.locations.iter().map(|l| l.text.clone()));
        body.extend(self.use_cases.clone());
        let body = body
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if body.chars().count() > 20000 {
            return Err("Recommendation is too long");
        }
        Ok((
            json!({"version":2,"origin":"user","entity_kind":self.entity_kind,
            "shelf":self.shelf()?,"summary":self.summary,"experience":self.experience,
            "attribution":self.attribution,"observations":self.observations,"locations":self.locations,
            "use_cases":self.use_cases,"classification":classification,"destination":self.destination}),
            body,
        ))
    }
    pub fn must_check_destination(&self, previous_subject: &str, previous: &Value) -> bool {
        self.destination.mode == "custom"
            && !self.destination_confirmed
            && previous["destination"]["mode"] == "custom"
            && previous["destination"]["url"] == self.destination.url
            && (self.subject != previous_subject
                || previous["entity_kind"] != self.entity_kind
                || previous["locations"] != json!(self.locations))
    }
}
