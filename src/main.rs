use sha2::{Sha256, Digest};
use secp256k1::{Secp256k1, SecretKey, PublicKey};
use tiny_keccak::{Hasher, Keccak};
use rand::Rng;
use std::thread;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use reqwest::Client;
use serde_json::json;
use serde::{Serialize, Deserialize};
use crossbeam_channel::{unbounded, Sender, Receiver};
use std::fs::OpenOptions;
use std::io::Write;
use std::collections::HashSet;

// ================= STRUCT =================

#[derive(Serialize, Deserialize, Debug)]
struct ApiResult {
    address: String,
    seed1: i64,
    seed2: i64,
    response: String,
}

// ================= BUILD CLIENT =================

fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| Client::new())
}

// ================= GENERATE ADDRESS =================

fn generate_address(address: &str, seed1: i64, seed2: i64) -> String {
    let mut buf = Vec::new();

    buf.extend_from_slice(address.to_lowercase().as_bytes());
    buf.extend_from_slice(&seed1.to_be_bytes());
    buf.extend_from_slice(&seed2.to_be_bytes());

    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let hash = hasher.finalize();

    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&hash);

    if pk_bytes.iter().all(|&b| b == 0) {
        pk_bytes[31] = 1;
    }

    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&pk_bytes)
        .unwrap_or_else(|_| SecretKey::from_slice(&[1u8; 32]).unwrap());

    let pubkey = PublicKey::from_secret_key(&secp, &sk);
    let pubkey_serialized = pubkey.serialize_uncompressed();

    let mut keccak = Keccak::v256();
    let mut output = [0u8; 32];
    keccak.update(&pubkey_serialized[1..]);
    keccak.finalize(&mut output);

    let addr = &output[12..];
    format!("0x{}", hex::encode(addr))
}

// ================= ASYNC HTTP =================

async fn send_async(
    client: Client,
    address: String,
    seed1: i64,
    seed2: i64,
    sender: Sender<ApiResult>,
) {
    match client
        .post("http://52.44.108.84:8084/new/record")
        .json(&json!({
            "address": address,
            "seed1": seed1,
            "seed2": seed2
        }))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let _ = sender.send(ApiResult {
                address,
                seed1,
                seed2,
                response: format!("[{}] {}", status.as_u16(), text),
            });
        }
        Err(err) => {
            let _ = sender.send(ApiResult {
                address,
                seed1,
                seed2,
                response: format!("ERROR: {}", err),
            });
        }
    }
}

// ================= WRITER =================

fn start_writer(rx: Receiver<ApiResult>) {
    thread::spawn(move || {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("results.json")
            .unwrap();

        for data in rx {
            let line = serde_json::to_string(&data).unwrap();
            writeln!(file, "{}", line).unwrap();
        }
    });
}

// ================= WORKER =================

fn worker(
    address: String,
    sender: Sender<ApiResult>,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = build_client();

    let prefix = "a1b7c";

    let mut sent_cache: HashSet<String> = HashSet::new();

    loop {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let seed1 = now + rand::thread_rng().gen_range(0..1000);

        for seed2 in 0..100000 {
            let gen = generate_address(&address, seed1, seed2);

            let addr = gen.replace("0x", "");
            let first10 = &addr[..10];

            if first10.contains(prefix) && !sent_cache.contains(&addr) {
                sent_cache.insert(addr.clone());

                println!("FOUND {} {} {}", addr, seed1, seed2);

                let client_clone = client.clone();
                let addr_clone = address.clone();
                let sender_clone = sender.clone();

                rt.spawn(send_async(
                    client_clone,
                    addr_clone,
                    seed1,
                    seed2,
                    sender_clone,
                ));
            }
        }
    }
}

// ================= MAIN =================

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: ./aibtc-rust <address>");
        return;
    }

    let address = args[1].clone();

    let cpu = num_cpus::get();
    let workers = 2;

    println!("CPU: {}", cpu);
    println!("Workers: {}", workers);
    println!("Mode: FULL POWER - no rate limit");

    let (tx, rx) = unbounded();

    // Hapus Arc<Mutex<RateLimiter>> — tidak dipakai lagi
    let _unused: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

    start_writer(rx);

    let mut handles = vec![];

    for _ in 0..workers {
        let addr_clone = address.clone();
        let tx_clone = tx.clone();

        let handle = thread::spawn(move || {
            worker(addr_clone, tx_clone);
        });

        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }
}
