use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub db_path: String,
    pub internal_api_token: String,

    pub bind_addr: SocketAddr,
    pub target_mac: [u8; 6],
    pub machine_ip: IpAddr,
    pub machine_check_port: u16,
    pub factorio_check_port: u16,
    pub tcp_timeout_ms: u64,
}

impl AppConfig {
    pub fn machine_check_addr(&self) -> SocketAddr {
        SocketAddr::new(self.machine_ip, self.machine_check_port)
    }
    pub fn factorio_check_addr(&self) -> SocketAddr {
        SocketAddr::new(self.machine_ip, self.factorio_check_port)
    }

    #[cfg(test)]
    pub fn test(db_path: String) -> Self {
        Self {
            db_path,
            internal_api_token: "test-token".to_string(),
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            target_mac: [0, 0, 0, 0, 0, 0],
            machine_ip: "127.0.0.1".parse().unwrap(),
            machine_check_port: 22,
            factorio_check_port: 10000,
            tcp_timeout_ms: 200,
        }
    }
}

pub fn load_config() -> AppConfig {
    AppConfig {
        db_path: std::env::var("DB_PATH")
            .unwrap_or_else(|_| "./factorio.db".to_string()),

        internal_api_token: std::env::var("INTERNAL_API_TOKEN")
            .expect("INTERNAL_API_TOKEN must be set"),

        bind_addr: std::env::var("BIND_ADDR")
            .expect("BIND_ADDR must be set")
            .parse()
            .expect("Invalid BIND_ADDR"),

        target_mac: parse_mac(
            &std::env::var("TARGET_MAC")
                .expect("TARGET_MAC must be set (format: 00:00:00:00:00:00)"),
        ),

        machine_ip: std::env::var("MACHINE_IP")
            .expect("MACHINE_IP must be set")
            .parse()
            .expect("Invalid MACHINE_IP"),
        machine_check_port: std::env::var("MACHINE_CHECK_PORT")
            .expect("MACHINE_CHECK_PORT must be set").parse().unwrap(),
        factorio_check_port: std::env::var("FACTORIO_CHECK_PORT")
            .expect("FACTORIO_CHECK_PORT must be set").parse().unwrap(),

        tcp_timeout_ms: std::env::var("TCP_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(200),
    }
}

fn parse_mac(input: &str) -> [u8; 6] {
    let parts: Vec<&str> = input.split(':').collect();

    if parts.len() != 6 {
        panic!("Invalid MAC address format");
    }

    let mut mac = [0u8; 6];

    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16)
            .unwrap_or_else(|_| panic!("Invalid MAC segment: {}", part));
    }

    mac
}