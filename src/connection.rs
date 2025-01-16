pub struct Connection {  
    state: State
}




pub enum State {
    Initiated,
    AuthDone,
    PendingResponse
}

pub enum Direction {
    C2S, S2C
}