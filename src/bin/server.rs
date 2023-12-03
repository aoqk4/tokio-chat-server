use anyhow::Result;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener, TcpStream,
    },
    sync::{
        broadcast::{self},
        Mutex,
    },
};
use tracing::instrument;
use tracing::{info, warn};

use tokio_chat_server::db_tiberius;

// 파라미터로 두기엔 너무 길어서 struct화
#[derive(Debug)]
struct BroadCastInfo {
    tx: broadcast::Sender<(String, SocketAddr)>,
    rx: broadcast::Receiver<(String, SocketAddr)>,
}

// 생성 impl
impl BroadCastInfo {
    fn new(
        tx: broadcast::Sender<(String, SocketAddr)>,
        rx: broadcast::Receiver<(String, SocketAddr)>,
    ) -> Self {
        Self { tx, rx }
    }
}

/// ## Title :
///
///     DB 체크해서 개인한테 보내주고 전체에게 얘 로그인 했다고 알린다.
///
/// ## Parameters :
///
///     tcprw: (&mut WriteHalf<'a>, &mut BufReader<ReadHalf<'a>>, &SocketAddr)
///     분리된 쓰는 소켓, 읽는 소켓 tuple, 그리고 그 소켓 IP:Port 정보
///
///     text: &mut String
///     텍스트 저장할 버퍼
///
///     broadcast_info:
///     &broadcast::Sender<(String, SocketAddr)>,
///     &mut broadcast::Receiver<(String, SocketAddr)>
///
///     broadcast Sendor와 Reciver 정보
///
///
/// ## Return :
///
///     로그인 유무에 따른 유저 아이디를 리턴한다. 아니면 빈값이거나 에러
///     Aysnc Result<String, anyhow::Error>
///
/// ## 수정 내역 :
///
///     23.11.24
///     아이디 비빌번호 가져와서 체크하고 전체문자 보내는 것 까지 init
///
///     23.11.25
///     입력된 사용자 ID로 리턴하는 것으로 변경 함
///     사용자 ID 틀리면 다시 입력하게 해줌
///
///     23.11.30
///     함수 파라미터 수 줄이기 위한 공통 분야 tuple화
///     tx 정도만 받게해서 상대에게만 알리게 할 수 있도록 한다..
///
async fn check_db_logic<'a>(
    tcprw: (
        &mut WriteHalf<'a>,
        &mut BufReader<ReadHalf<'a>>,
        &SocketAddr,
    ),
    text: &mut String,
    tx: broadcast::Sender<(String, SocketAddr)>,
    rx: &mut broadcast::Receiver<(String, SocketAddr)>,
) -> Result<String, anyhow::Error> {
    // 일반 소켓 writer, reader 분리 해준다.
    let (writer, reader, addr) = tcprw;

    // 아이디를 입력해 주세요 -> todo!(space 비밀번호 식으로 해서 같이 받자.)
    writer
        .write_all("아이디를 입력하여 주세요\n".as_bytes())
        .await?;

    // bufreader 입히고
    let mut reader = BufReader::new(reader);

    // 로그인정보 입력받는다.
    reader.read_line(text).await?;

    // 로그인 체크 함수 call 해주고
    let mut login_bool = db_tiberius((text.trim(), "1234")).await?;

    // 로그인 루프 시작
    loop {
        // true면
        if login_bool {
            // 처음 입장 할 때 전체에게 인사 ?
            tx.send((
                format!("어서오세요 {}\n", text.trim()).to_string(),
                *addr,
            ))
            .unwrap();

            // 전체한테 전달하고
            let (msg, _) = rx.recv().await.unwrap();

            // 내용 뿌린다.
            writer
                .write_all(format!("{} : {}", text.trim(), msg).as_bytes())
                .await
                .unwrap();

            // 로그인 루프 종료

            return Ok(text.clone());
        }
        // 아니면 다시 물어본다 -> 재귀로 하면 오버플로 날듯?
        else {
            warn!("사용자가 잘못 입력한 듯? 사용자 id : {}", text);

            // 사용자가 다시 입력하라고 한다.
            writer
                .write_all("아이디 혹은 비밀번호 잘못 입력하신듯?\n".as_bytes())
                .await?;

            // 문자 초기화 해주고
            text.clear();

            // 다시 입력하게 해준다.
            reader.read_line(text).await?;

            // 그리고 받은 정보로 다시 반영하고
            login_bool = db_tiberius((&text.trim(), &"1234")).await?;

            // continue;
            continue;
        }
    }
}

/// ## Title :
///
///     쓰레드 돌리는 비동기 채팅 서버 루프 서버
///     Select! 매크로 포함한 비동기
///
/// ## Parameters :
///
///     broadcast_info: BroadCastInfo
///     channal::broadcast Sender, Reiciver 스트럭쳐
///
///     socket: &mut TcpStream
///     TCP 소켓
///
///     addr
///     IP:Port 정보
///
///     mutex_handler_clone
///     쓰레드 공유 정보(여기서는 id Hashmpa)
///
///
/// ## Return :
///
///     에러 처리 위한 Result
///     Result<(), anyhow::Error>
///
/// ## 수정 내역 :
///
///     23.11.30
///     메인에 때려 박았더니 코드가 인읽혀서 코드 분리 함
///
async fn chat_server_handler(
    broadcast_info: BroadCastInfo,
    socket: &mut TcpStream,
    addr: SocketAddr,
    mutex_handler_clone: Arc<Mutex<HashMap<SocketAddr, String>>>,
) -> Result<(), anyhow::Error> {
    // 소켓의 역할을 나눈다 -> 소유권을 한 놈이 다가져 가는 것을 막는다.
    let (reader, mut writer) = socket.split();

    // BUFREADER를 통해 READHALF에 있는 버퍼를 읽어 들인다고 선언한다.
    let mut reader = BufReader::new(reader);

    // 버퍼 메시지를 저정할 공간
    let mut line = String::new();

    // tx rx 만들어준다.
    let tx = broadcast_info.tx;
    let mut rx = broadcast_info.rx;

    // 로그인 체크 Result 만들어 주고
    let login_res = check_db_logic(
        (&mut writer, &mut reader, &addr),
        &mut line,
        tx.clone(),
        &mut rx,
    )
    .await;

    // 에러 핸들링 위한 match
    match login_res {
        // 소유권 뺏기면 안되니깐 ref로 Option 만들어 주고
        Ok(ref res) => {
            // 빈 버퍼(ID 가 없어요 나올 때(현재로서는 나오진 않겠다만..))
            if res.is_empty() {
                return Ok(());
            }
            // match가 끝나면 알아서 해제되게 안에서 락 걸어주고
            let mut id_handler = mutex_handler_clone.lock().await;

            // 넣는다.
            id_handler.insert(addr, login_res.unwrap().trim().to_string());
        }
        // 에러처리
        Err(err) => {
            warn!("ID PW 입력 중 에러 발생 ! err : {}", err);
            return Ok(());
        }
    }
    // ID 처리 끝났으니깐 한번 비워준다.
    line.clear();

    // 버퍼 핸들링 루프 시작
    loop {
        // js의 then 문법과 유사 but, 여기서는 여러 비동기 중에
        // 하나가 완료 되면 나머지 작업을 하게된다.
        // 둘 중 하나가 실행되게 되면, 나머지도 타이밍을 봐서 실행하게 됨
        tokio::select! {
            // 리더로 엔터 전까지 한줄을 읽는다. 그걸 메시지 저장공간에 저장한다.
            res = reader.read_line(&mut line) => {
                if res.unwrap() == 0 {
                    return Ok(());
                }
                // 메시지 버퍼와 IP 주소를 보낸다.
                tx.send((line.clone(), addr)).unwrap();
                // 메시지 공간 청소
                line.clear();
            }
            // RECEIVE 받는다. (지금까지 버퍼에 저장된 값을 읽겠다.)
            res = rx.recv() => {
                let (msg, other_addr) = res.unwrap();

                // ID 가져올라고 락 걸어주고
                let mut id_handler = mutex_handler_clone.lock().await;

                // 만약에 나가고 싶다 하면
                if msg.trim() == "IWANTEXIT" {
                    // 특정 아이디 있는지 한번 체크해주고
                    if let Some(txt) = id_handler.remove(&other_addr) {
                        // 로그 뿌리고
                        info!("{} 님이 나가셨어요.\n", txt);

                        // 다른 사람들에게는 로그아웃 했다고 알려준다.
                        tx.send((format!("{} 님이 나가셨어요.\n", txt), addr)).unwrap();
                    }
                }
                // 만약 로그인 정보가 없으면?
                else if id_handler.get(&other_addr).is_none() && addr != other_addr {
                    // 로그아웃 되었다는 걸로 취급하고 일단 그대로 보낸다.
                    writer.write_all(msg.as_bytes()).await.unwrap();
                }
                // IP 주소 확인해서 내 정보가 아니면? -> 메시지를 출력한다.
                else if addr != other_addr {
                    // 메시지 보내준다.
                    writer.write_all(format!("{} : {}",id_handler.get(&other_addr).unwrap() ,msg).as_bytes()).await.unwrap();
                }
            }
        }
    }
}

#[instrument]
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    // IP:PORT 주소 묶는다.
    let listener = TcpListener::bind("127.0.0.1:8090").await?;

    // channel의 정확한 타입 추론 불가능 하다면? -> turbofish 문법으로 타입 추정 가능캐 하기
    // 채널 생성한다.
    let (tx, mut _rx) = broadcast::channel::<(String, SocketAddr)>(10);

    // (현재 로그인 되고 채팅치고 있는) 유저 표시 위한 아이디 핸들링 Arc 생성 ->
    // async 쓰레드여서 반드시 비동기로
    let arc_id_handler = Arc::new(Mutex::new(HashMap::<SocketAddr, String>::new()));

    // 루프 시작
    loop {
        // 클라이언트 접속을 허가한다 -> SOCKET 받아온다.
        let (mut socket, addr) = listener.accept().await?;

        // tx를 하나 클론 떠온다.
        let tx: broadcast::Sender<(String, SocketAddr)> = tx.clone();

        // 각 clone된 tx에 대한 rx를 구독해 주어야 한다(같은 tx이긴 하다...).
        let rx: broadcast::Receiver<(String, SocketAddr)> = tx.subscribe();

        // 각 쓰레드 루프마다 사용 할 수 있게 떠주고
        let arc_id_handler_clone = Arc::clone(&arc_id_handler);

        // 브로드캐스트 구조체 생성
        let broadcast_info = BroadCastInfo::new(tx, rx);

        // 스레드 생성
        tokio::spawn(async move {
            // 채널 서버 시작
            chat_server_handler(broadcast_info, &mut socket, addr, arc_id_handler_clone)
                .await
                .unwrap();
        });
    }
}
