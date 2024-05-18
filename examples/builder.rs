use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use derive_builder::Builder;

#[allow(unused)]
#[derive(Debug, Builder)]
// #[builder(pattern = "owned")]
#[builder(build_fn(name = "_priv_build"))]
struct User {
    #[builder(setter(into))]
    name: String,

    #[builder(setter(into, strip_option), default)]
    email: Option<String>,
    #[builder(setter(custom))]
    dob: DateTime<Utc>,
    #[builder(setter(skip))]
    age: u32,
    #[builder(default = "vec![]", setter(each(name = "skill", into)))]
    skills: Vec<String>,
}
fn main() -> Result<()> {
    let user = User::build()
        .name("Alice")
        .skill("C++")
        .skill("Rust")
        .email("415074476@qq.com")
        .dob("2021-08-01T00:00:00Z")
        .build()?;
    println!("{:?}", user);
    Ok(())
}

impl User {
    fn build() -> UserBuilder {
        UserBuilder::default()
    }
}

impl UserBuilder {
    pub fn build(&self) -> Result<User> {
        let mut user = self._priv_build()?;
        user.age = (Utc::now().year() - user.dob.year()) as _;
        Ok(user)
    }

    pub fn dob(&mut self, dob: &str) -> &mut Self {
        self.dob = Some(
            DateTime::parse_from_rfc3339(dob)
                .unwrap()
                .with_timezone(&Utc),
        );

        self
    }
}
