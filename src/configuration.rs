use std::convert::{TryFrom, TryInto};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub ecosystem: EcosystemSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub addr_host: String,
    pub addr_base_port: usize,
}

#[derive(serde::Deserialize)]
pub struct EcosystemSettings {
    pub population_size: usize,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let mut settings = config::Config::default();
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration");

    // read the 'default' configuration file
    settings.merge(config::File::from(configuration_directory.join("base")).required(true))?;

    // detect the running environment
    // default to 'local' if unspecified
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    // layer on the environment specific values
    settings.merge(
        config::File::from(configuration_directory.join(environment.as_str())).required(true),
    )?;

    // Add in settings from environment variables (with a prefix of APP and '__' as separator)
    // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
    settings.merge(config::Environment::with_prefix("app").separator("__"))?;

    settings.try_into()
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_can_be_loaded() -> Result<(), config::ConfigError> {
        let settings = get_configuration()?;
        assert_eq!(settings.application.addr_host, "[::1]");
        assert_eq!(settings.application.addr_base_port, 1000);
        assert_eq!(settings.ecosystem.population_size, 2);

        Ok(())
    }
}
