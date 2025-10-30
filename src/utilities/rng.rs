use rand::rngs::ThreadRng;

pub fn get_rng() -> ThreadRng {
    rand::thread_rng()
}
