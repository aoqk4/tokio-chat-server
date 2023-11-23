use std::net::SocketAddr;

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::broadcast,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv()?;

    // IP:PORT 주소 묶는다.
    let listener = TcpListener::bind("127.0.0.1:8090").await?;

    // channel의 정확한 타입 추론 불가능 하다면? -> turbofish 문법으로 타입 추정 가능캐 하기
    // 채널 생성한다.
    let (tx, mut _rx) = broadcast::channel::<(String, SocketAddr)>(10);

    // 루프 시작
    loop {
        // 클라이언트 접속을 허가한다 -> SOCKET 받아온다.
        let (mut socket, addr) = listener.accept().await?;

        tx.send((format!("{} IP is comming!\n", addr).to_string(), addr))?;

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

            // 버퍼 핸들링 루프 시작
            loop {
                // js의 then 문법과 유사 but, 여기서는 여러 비동기 중에
                // 하나가 완료 되면 나머지 작업을 하게된다.
                // 둘 중 하나가 실행되게 되면, 나머지도 타이밍을 봐서 실행하게 됨
                tokio::select! {
                    // 리더로 엔터 전까지 한줄을 읽는다. 그걸 메시지 저장공간에 저장한다.
                    res = reader.read_line(&mut line) => {
                        if res.unwrap() == 0 {
                            break;
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
