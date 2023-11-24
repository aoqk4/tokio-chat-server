use anyhow::Result;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener,
    },
    sync::broadcast::{self},
};
use tokio_chat_server::db_tiberius;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // IP:PORT 주소 묶는다.
    let listener = TcpListener::bind("127.0.0.1:8090").await?;

    // channel의 정확한 타입 추론 불가능 하다면? -> turbofish 문법으로 타입 추정 가능캐 하기
    // 채널 생성한다.
    let (tx, mut _rx) = broadcast::channel::<(String, SocketAddr)>(10);

    // 루프 시작
    loop {
        // 클라이언트 접속을 허가한다 -> SOCKET 받아온다.
        let (mut socket, addr) = listener.accept().await?;

        // tx를 하나 클론 떠온다.
        let tx = tx.clone();

        // 각 clone된 tx에 대한 rx를 구독해 주어야 한다(같은 tx이긴 하다...).
        let mut rx = tx.subscribe();

        // 스레드 생성
        tokio::spawn(async move {
            // 소켓의 역할을 나눈다 -> 소유권을 한 놈이 다가져 가는 것을 막는다.
            let (reader, mut writer) = socket.split();

            // BUFREADER를 통해 READHALF에 있는 버퍼를 읽어 들인다고 선언한다.
            let mut reader = BufReader::new(reader);

            // 버퍼 메시지를 저정할 공간
            let mut line = String::new();

            check_db_logic(&mut writer, &mut reader, &mut line, &tx, &mut rx, &addr)
                .await
                .unwrap();

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
                            return Ok::<(), anyhow::Error>;
                        }
                        // 메시지 버퍼와 IP 주소를 보낸다.
                        tx.send((line.clone(), addr)).unwrap();

                        // 메시지 공간 청소
                        line.clear();
                    }

                    // RECEIVE 받는다. (지금까지 버퍼에 저장된 값을 읽겠다.)
                    res = rx.recv() => {
                        let (msg, other_addr) = res.unwrap();

                        // IP 주소 확인해서 내 정보가 아니면? -> 메시지를 출력한다.
                        if addr != other_addr {
                            writer.write_all(format!("{} : {}",other_addr, msg).as_bytes()).await.unwrap();
                        }
                    }
                }
            }
        });
    }
}
/// ## Title : 
/// 
///     DB 체크해서 개인한테 보내주고 전체에게 얘 로그인 했다고 알린다.
///
/// ## Parameters : 
/// 
///     writer: &mut WriteHalf<'a>
///     쓰는 소켓(reader와 같은 수명)
/// 
///     reader: &mut BufReader<ReadHalf<'a>>
///     읽는 소켓(writer와 같은 수명) 
/// 
///     text: &mut String
///     텍스트 저장할 버퍼
/// 
///     tx: &broadcast::Sender<(String, SocketAddr)>
///     broadcast channel Sender 전체 문자 보낼 sender
/// 
///     rx: &mut broadcast::Receiver<(String, SocketAddr)>
///     broadcast channel Receiver 전체 문자 받아서 뿌려줄 receiver
/// 
///     addr: &SocketAddr 
///     유저 IP:Port 정보
///
/// ## Return : 
/// 
///     로그인 성공인지 실패인지에 대한 db 처리 결과를 그대로 보낸다.
///     Aysnc Result<bool, anyhow::Error>
/// 
/// ## 수정 내역 :
/// 
///     23.11.24
///     아이디 비빌번호 가져와서 체크하고 전체문자 보내는 것 까지 init
async fn check_db_logic<'a>(
    writer: &mut WriteHalf<'a>,
    reader: &mut BufReader<ReadHalf<'a>>,
    text: &mut String,
    tx: &broadcast::Sender<(String, SocketAddr)>,
    rx: &mut broadcast::Receiver<(String, SocketAddr)>,
    addr: &SocketAddr,
) -> Result<bool, anyhow::Error> {
    // 아이디를 입력해 주세요 -> todo!(space 비밀번호 식으로 해서 같이 받자.) 
    writer
        .write_all("아이디를 입력하여 주세요\n".as_bytes())
        .await?;

    // bufreader 입히고
    let mut reader = BufReader::new(reader);

    // 로그인정보 입력받는다.
    reader.read_line(text).await?;

    // 로그인 체크 함수 call 해주고
    let login_bool = db_tiberius(&text.trim().to_string(), &"1234".to_string()).await?;

    // true면
    if login_bool  {
        // 처음 입장 할 때 전체에게 인사 ?
        tx.send((
            format!("{} 님이 입장 하였습니다.\n", addr).to_string(),
            *addr,
        ))
        .unwrap();

        // 전체한테 전달하고
        let (msg, other_addr) = rx.recv().await.unwrap();

        // 내용 뿌린다.
        writer
            .write_all(format!("{} : {}", other_addr, msg).as_bytes())
            .await
            .unwrap();
    }
    
    // 최종 로그인 정보 리턴해준다 -> todo!(나중에는 아이디 정보 넘겨주는 걸로 해서 표시나 로그아웃 가능하게)
    Ok(login_bool)
}

// async fn broadcast_all_of_them() {}
