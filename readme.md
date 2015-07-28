# Game Udp
[![Build Status](https://travis-ci.org/acmcarther/rust_game_udp.svg?branch=master)](https://travis-ci.org/acmcarther/rust_game_udp)

## Protocol level Headers


|App ID   |sequence #|curr ack #|past 32 acks|payload|
|:-------:|:--------:|:--------:|:----------:|:-----:|
|[u32; 3] |u16       |u16       |u32         |payload|
|12b      |2b        |2b        |4b          |236b   |


Recv-thread:
  - Identify app id
  - Identify headers
  - Add seq# to own acks for SocketAddr
  - Record remote acks for SocketAddr
  - Handle ACKS
    - Send Send-Thread(send_acks_tx) own acks
    - Identify dropped packets(no ack after falling off buffer)
      - Send Send-Thread(send_msg_tx) dropped packets for resubmit
  - Send payload to application

Send-Thread:
  - try_recv own acks (and bundle results into updated_acks)
  - recv from send_msg_rx
  - Increment seq #
  - Add proper headers
    - App Id
    - Sequence #
    - Current Ack (from updated_acks)
    - Past Acks (from updated_acks)
  - Send Payload

## Application Headers (TBD)
|message type|payload|
|:----------:|:-----:|
|u8          |payload|
|1b          |235b   |

