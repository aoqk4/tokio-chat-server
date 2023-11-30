use anyhow::Result;
use tiberius::{AuthMethod, Client, Config, Query};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use tracing::instrument;
use tracing::{info, warn};

/// ## Title:
///
///     tiberius 설정 파일 생성
///
/// ## Parameters :
///
///     ip: &str
///     IP 주소
///
///     port: u16
///     포트 주소
///
/// ## Return :
///
///     DB 설정값에 대한 Result
///     Aysnc Result<bool, anyhow::Error>
///
/// ## 수정 내역 :
///
///     23.11.30
///     DB 설정 함수 분리, 약간의 로깅 추가
///
async fn config_db(ip: &str, port: u16, ig_creted: bool) -> Result<Config, anyhow::Error> {
    // ENV에 DB 정보 가져온다.
    dotenvy::dotenv()?;

    // DB 정보란 init
    let mut config = Config::new();

    // IP:port 정보 입력해준다.
    config.host(ip);
    config.port(port);

    // 보안정보(아이디 비밀번호) 입력해준다.
    config.authentication(AuthMethod::sql_server(
        dotenvy::var("USER_ID")?,
        dotenvy::var("USER_PW")?,
    ));

    if ig_creted {
        // 인증서 무시한다는 뜻
        config.trust_cert();
    }

    info!("DB 설정 성공");

    Ok(config)
}

/// ## Title:
///
///     DB와의 연결
///
/// ## Parameters :
///
///     config: tiberius::Config
///     DB 관련 설정
///
///
/// ## Return :
///
///     DB 연결에 대한 client(tcp연결 형식) Result
///     Aysnc Result<Client<Compat<TcpStream>>, anyhow::Error>
///
/// ## 수정 내역 :
///
///     23.11.30
///     DB 연결 함수 분리, 약간의 로깅 추가
///
async fn connect_db(config: Config) -> Result<Client<Compat<TcpStream>>, anyhow::Error> {
    // DB Tcp 연결 위한 요청 해준다.
    let tcp = TcpStream::connect(config.get_addr()).await?;

    // Nagle Algo를 무시한다.(스트림을 있는 그래로 기다리지 않고 보낸다.)
    // Nagle Algo는 https://devjh.tistory.com/106 참조 함.
    tcp.set_nodelay(true)?;

    // DB 주소로 TCP 연결 해준다.
    let client = Client::connect(config, tcp.compat_write()).await?;

    info!("DB와의 연결 성공");

    Ok(client)
}

/// ## Title:
///
///     tiberius 를 통한 DB 체크
///
/// ## Parameters :
///
///     user_id: &string
///     유저 아이디
///
///     user_pw: &string
///     유저 비밀번호
///
/// ## Return :
///
///     로그인 성공인지 실패인지에 대한 결과
///     Aysnc Result<bool, anyhow::Error>
///
/// ## 수정 내역 :
///
///     23.11.24
///     아이디 비빌번호 가져오는 기본적인 함수 init
///
///     23.11.25
///     로깅 하는거 추가
///     Row가 None값일 때 핸들링 추가
#[instrument]
pub async fn db_tiberius(user_info: (&str, &str)) -> Result<bool, anyhow::Error> {
    let config = config_db("127.0.0.1", 1433, true).await?;

    let mut client = connect_db(config).await?;

    // 데이터 가져오기 위한 결과 파라미터 매칭 (숫자로 해도 상관은 없나보다. 근데 좀더 명확하게 표현하려고)
    let params = vec![String::from("CHAT_USER"), String::from("CHAT_PW")];

    let (user_id, user_pw) = user_info;

    // 적용할 sql문
    let sql_query_string = format!(
        "SELECT CHAT_USER, CHAT_PW \
        FROM    CHAT_SERVER.dbo.TB_CHAT_USER \
        WHERE   CHAT_USER = '{}' \
        AND     CHAT_PW = '{}'",
        user_id, user_pw
    );

    // 쿼리 만들어내고
    let mut select_query = Query::new(sql_query_string);

    // 파라미터 입혀서 데이터 가져올 수 있게 하고
    for param in params {
        select_query.bind(param);
    }

    // excute 한다.
    let res = select_query.query(&mut client).await.unwrap().into_row();

    match res.await {
        Ok(res) => {
            // row 데이터 가져오면
            if res.is_none() {
                warn!("DB에 해당 사항이 없음");
                return Ok(false);
            }
            let row = res.unwrap();

            // 데이터 있는지 체크하고 리턴
            if row.len() > 0 {
                // 서버 로깅
                info!("UserName: {} 의 DB 처리 성공!", user_id);
                return Ok(true);
            }

            Ok(false)
            // 서버 로깅
        }
        Err(err) => {
            warn!("DB 상의 에러 에러 : {}", err);
            Ok(false)
        }
    }
}
