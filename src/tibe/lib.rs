use anyhow::Result;
use tiberius::{AuthMethod, Client, Config, Query};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::{info, warn};
use tracing::instrument;

/// ## Title:
///
///     tiberius 를 통한 로그인 체크
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
#[instrument]
pub async fn db_tiberius(user_id: &String, user_pw: &String) -> Result<bool, anyhow::Error> {
    // ENV에 DB 정보 가져온다.
    dotenvy::dotenv().unwrap();

    // DB 정보란 init
    let mut config = Config::new();

    // IP:port 정보 입력해준다.
    config.host("127.0.0.1");
    config.port(1433);

    // 보안정보(아이디 비밀번호) 입력해준다.
    config.authentication(AuthMethod::sql_server(
        dotenvy::var("USER_ID").unwrap(),
        dotenvy::var("USER_PW").unwrap(),
    ));

    // 인증서 무시한다는 뜻
    config.trust_cert();

    // DB Tcp 연결 위한 요청 해준다.
    let tcp = TcpStream::connect(config.get_addr()).await?;

    // Nagle Algo를 무시한다.(스트림을 있는 그래로 기다리지 않고 보낸다.)
    // Nagle Algo는 https://devjh.tistory.com/106 참조 함.
    tcp.set_nodelay(true)?;

    // DB 주소로 TCP 연결 해준다.
    let mut client = Client::connect(config, tcp.compat_write()).await?;

    // 데이터 가져오기 위한 결과 파라미터 매칭 (숫자로 해도 상관은 없나보다. 근데 좀더 명확하게 표현하려고)
    let params = vec![String::from("CHAT_USER"), String::from("CHAT_PW")];

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
            let row = res.unwrap();
            // 데이터 있는지 체크하고 리턴
            if row.len() > 0 {
                // 서버 로깅
                info!("UserName: {} 의 DB 처리 성공!", user_id);
                return Ok(true);
            }
            // 서버 로깅
            warn!("UserName: {} 가 DB에 없음", user_id);
            Ok(false)
        }
        Err(err) => {
            warn!("DB 상의 에러 에러 : {}", err);
            Ok(false)
        }
    }
}