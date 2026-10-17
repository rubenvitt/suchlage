//! S3 mit Signatur Version 4, nur was Suchlage braucht: `HEAD`, `GET` und `PUT` eines Objekts im
//! Path-Style (`<endpoint>/<bucket>/<key>`). Kein AWS-SDK: Das zöge einen zweiten HTTP- und
//! TLS-Stack herein. Die Signatur ist eine reine Funktion und gegen die AWS-Testvektoren geprüft.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// SHA-256 des leeren Bodys.
pub const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, Clone, PartialEq)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

/// Bucket an einem S3-Endpunkt.
#[derive(Debug, Clone, PartialEq)]
pub struct Bucket {
    /// Ohne Schrägstrich am Ende, z. B. `http://fotos:3900`.
    pub endpoint: String,
    pub name: String,
    pub region: String,
}

const BLOCK: usize = 64;

/// HMAC-SHA256 (RFC 2104).
fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let pad = |byte: u8| block.map(|b| b ^ byte);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(data)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .into()
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// URI-Kodierung nach AWS: alles außer `A–Z a–z 0–9 - _ . ~` (und `/` im Pfad) als `%XX`.
pub fn uri_encode(value: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved || (keep_slash && b == b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Eine zu signierende Anfrage. `path` ist schon kodiert, `headers` haben kleine Namen und
/// enthalten `host` und `x-amz-date`.
pub struct Canonical<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a [(String, String)],
    pub headers: &'a [(String, String)],
    pub payload_sha256: &'a str,
}

fn canonical_request(r: &Canonical) -> (String, String) {
    let mut query: Vec<(String, String)> = r
        .query
        .iter()
        .map(|(k, v)| (uri_encode(k, false), uri_encode(v, false)))
        .collect();
    query.sort();
    let query = query
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let mut headers: Vec<(String, String)> = r
        .headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    headers.sort();
    let signed = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let lines: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        r.method, r.path, query, lines, signed, r.payload_sha256
    );
    (request, signed)
}

/// Wert des `Authorization`-Headers.
pub fn authorization(
    r: &Canonical,
    creds: &Credentials,
    region: &str,
    service: &str,
    at: DateTime<Utc>,
) -> String {
    let (request, signed) = canonical_request(r);
    let date = at.format("%Y%m%d").to_string();
    let amz_date = at.format("%Y%m%dT%H%M%SZ").to_string();
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(request.as_bytes())
    );
    let key = [region, service, "aws4_request"].iter().fold(
        hmac(
            format!("AWS4{}", creds.secret_access_key).as_bytes(),
            date.as_bytes(),
        ),
        |key, part| hmac(&key, part.as_bytes()),
    );
    let signature = hex::encode(hmac(&key, to_sign.as_bytes()));
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}",
        creds.access_key_id
    )
}

fn unreachable(e: reqwest::Error) -> AppError {
    tracing::warn!(error = %e, "Fotospeicher nicht erreichbar");
    AppError::Unavailable("Fotospeicher nicht erreichbar".into())
}

/// Signierte Anfrage an ein Objekt; `body` mit Inhaltstyp nur bei `PUT`.
async fn send(
    http: &reqwest::Client,
    bucket: &Bucket,
    creds: &Credentials,
    method: reqwest::Method,
    key: &str,
    body: Option<(Vec<u8>, &str)>,
) -> Result<reqwest::Response, AppError> {
    let path = format!(
        "/{}/{}",
        uri_encode(&bucket.name, false),
        uri_encode(key, true)
    );
    let url = url::Url::parse(&format!("{}{path}", bucket.endpoint))
        .map_err(|_| AppError::Unavailable("S3-Adresse des Fotospeichers ungültig".into()))?;
    let host = match (url.host_str(), url.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        (None, _) => {
            return Err(AppError::Unavailable(
                "S3-Adresse des Fotospeichers ungültig".into(),
            ));
        }
    };
    let now = Utc::now();
    let payload = body
        .as_ref()
        .map_or(EMPTY_SHA256.to_string(), |(b, _)| sha256_hex(b));
    let mut headers = vec![
        ("host".to_string(), host),
        ("x-amz-content-sha256".to_string(), payload.clone()),
        (
            "x-amz-date".to_string(),
            now.format("%Y%m%dT%H%M%SZ").to_string(),
        ),
    ];
    if let Some((_, content_type)) = &body {
        headers.push(("content-type".to_string(), content_type.to_string()));
    }
    let auth = authorization(
        &Canonical {
            method: method.as_str(),
            path: &path,
            query: &[],
            headers: &headers,
            payload_sha256: &payload,
        },
        creds,
        &bucket.region,
        "s3",
        now,
    );
    let mut req = http.request(method, url).header("authorization", auth);
    for (k, v) in headers.into_iter().filter(|(k, _)| k != "host") {
        req = req.header(k, v);
    }
    if let Some((bytes, _)) = body {
        req = req.body(bytes);
    }
    req.send().await.map_err(unreachable)
}

fn refused(res: &reqwest::Response) -> AppError {
    tracing::warn!(status = %res.status(), "Fotospeicher lehnt ab");
    AppError::Unavailable(format!(
        "Fotospeicher lehnt ab (HTTP {})",
        res.status().as_u16()
    ))
}

/// Größe des Objekts oder `None`, wenn es fehlt.
pub async fn head(
    http: &reqwest::Client,
    bucket: &Bucket,
    creds: &Credentials,
    key: &str,
) -> Result<Option<u64>, AppError> {
    let res = send(http, bucket, creds, reqwest::Method::HEAD, key, None).await?;
    match res.status() {
        // Bei HEAD steht die Größe nur im Header; `content_length()` meint den (leeren) Body.
        s if s.is_success() => Ok(Some(
            res.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()?.parse().ok())
                .unwrap_or(0),
        )),
        reqwest::StatusCode::NOT_FOUND => Ok(None),
        _ => Err(refused(&res)),
    }
}

/// Antwort mit dem Objekt als Strom; `NotFound`, wenn es fehlt.
pub async fn get(
    http: &reqwest::Client,
    bucket: &Bucket,
    creds: &Credentials,
    key: &str,
) -> Result<reqwest::Response, AppError> {
    let res = send(http, bucket, creds, reqwest::Method::GET, key, None).await?;
    match res.status() {
        s if s.is_success() => Ok(res),
        reqwest::StatusCode::NOT_FOUND => Err(AppError::NotFound),
        _ => Err(refused(&res)),
    }
}

pub async fn put(
    http: &reqwest::Client,
    bucket: &Bucket,
    creds: &Credentials,
    key: &str,
    body: Vec<u8>,
    content_type: &str,
) -> Result<(), AppError> {
    let res = send(
        http,
        bucket,
        creds,
        reqwest::Method::PUT,
        key,
        Some((body, content_type)),
    )
    .await?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(refused(&res))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn hmac_matches_rfc_4231() {
        // Testfall 2
        assert_eq!(
            hex::encode(hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Testfall 6: Schlüssel länger als ein Block
        assert_eq!(
            hex::encode(hmac(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    /// `get-vanilla` aus der AWS-Testsuite für Signatur Version 4.
    #[test]
    fn signature_matches_the_aws_test_suite() {
        let creds = Credentials {
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
        };
        let at = Utc.with_ymd_and_hms(2015, 8, 30, 12, 36, 0).unwrap();
        let headers = [
            ("Host".to_string(), "example.amazonaws.com".to_string()),
            ("X-Amz-Date".to_string(), "20150830T123600Z".to_string()),
        ];
        let auth = authorization(
            &Canonical {
                method: "GET",
                path: "/",
                query: &[],
                headers: &headers,
                payload_sha256: EMPTY_SHA256,
            },
            &creds,
            "us-east-1",
            "service",
            at,
        );
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
    }

    #[test]
    fn keys_are_encoded_like_aws_expects() {
        assert_eq!(
            uri_encode("fotos/RC1/DJI 1_T.JPG", true),
            "fotos/RC1/DJI%201_T.JPG"
        );
        assert_eq!(uri_encode("a/b+c~", false), "a%2Fb%2Bc~");
        assert_eq!(uri_encode("ä", true), "%C3%A4");
    }

    /// Fake-S3: prüft Pfad, Signatur-Header und liefert ein Objekt.
    async fn fake_s3() -> String {
        use axum::extract::Request;
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        let app = axum::Router::new().fallback(|req: Request| async move {
            let auth = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string();
            if !auth.starts_with("AWS4-HMAC-SHA256 Credential=GK1/")
                || !auth.contains("host;x-amz-content-sha256;x-amz-date, Signature=")
            {
                return StatusCode::FORBIDDEN.into_response();
            }
            match (req.method().as_str(), req.uri().path()) {
                ("HEAD", "/fotos/fotos/RC1/DJI_1_T.JPG") => {
                    ([("content-length", "3")], "").into_response()
                }
                ("GET", "/fotos/fotos/RC1/DJI_1_T.JPG") => "jpg".into_response(),
                ("PUT", "/fotos/fotos/RC1/neu.jpg") => StatusCode::OK.into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn head_get_and_put_reach_the_object_with_a_signature() {
        let bucket = Bucket {
            endpoint: fake_s3().await,
            name: "fotos".into(),
            region: "garage".into(),
        };
        let creds = Credentials {
            access_key_id: "GK1".into(),
            secret_access_key: "geheim".into(),
        };
        let http = reqwest::Client::new();
        assert_eq!(
            head(&http, &bucket, &creds, "fotos/RC1/DJI_1_T.JPG")
                .await
                .unwrap(),
            Some(3)
        );
        assert_eq!(
            head(&http, &bucket, &creds, "fotos/RC1/fehlt.jpg")
                .await
                .unwrap(),
            None
        );
        let body = get(&http, &bucket, &creds, "fotos/RC1/DJI_1_T.JPG")
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(&body[..], b"jpg");
        assert!(matches!(
            get(&http, &bucket, &creds, "fotos/RC1/fehlt.jpg").await,
            Err(AppError::NotFound)
        ));
        put(
            &http,
            &bucket,
            &creds,
            "fotos/RC1/neu.jpg",
            b"x".to_vec(),
            "image/jpeg",
        )
        .await
        .unwrap();
        let wrong = Credentials {
            access_key_id: "GK2".into(),
            ..creds
        };
        assert!(matches!(
            head(&http, &bucket, &wrong, "fotos/RC1/DJI_1_T.JPG").await,
            Err(AppError::Unavailable(_))
        ));
    }
}
