use std::future::Future;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::{mem, ptr};

use foreign_types::{foreign_type, ForeignType};
use libc::{c_int, c_void};
use openssl::error::ErrorStack;

mod ffi;

pub fn async_is_capable() -> bool {
    let capable = unsafe { ffi::ASYNC_is_capable() };
    capable == 1
}

pub fn async_thread_init(max_size: usize, init_size: usize) -> Result<(), ErrorStack> {
    let r = unsafe { ffi::ASYNC_init_thread(max_size, init_size) };
    if r == 1 {
        Ok(())
    } else {
        Err(ErrorStack::get())
    }
}

pub fn async_thread_cleanup() {
    unsafe { ffi::ASYNC_cleanup_thread() }
}

foreign_type! {
    ///
    type CType = ffi::ASYNC_WAIT_CTX;
    fn drop = ffi::ASYNC_WAIT_CTX_free;

    pub struct AsyncWaitCtx;
    pub struct AsyncWaitCtxRef;
}

impl AsyncWaitCtx {
    fn new() -> Result<Self, ErrorStack> {
        let wait_ctx = unsafe { ffi::ASYNC_WAIT_CTX_new() };
        if wait_ctx.is_null() {
            return Err(ErrorStack::get());
        }
        Ok(AsyncWaitCtx(wait_ctx))
    }

    pub fn get_all_fds(&self) -> Result<Vec<RawFd>, ErrorStack> {
        let mut fd_count = 0usize;
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_all_fds(self.0, ptr::null_mut(), &mut fd_count as *mut usize)
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        let mut fds: Vec<c_int> = vec![0; fd_count];
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_all_fds(self.0, fds.as_mut_ptr(), &mut fd_count as *mut usize)
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        Ok(fds.into_iter().map(RawFd::from).collect())
    }

    pub fn get_changed_fds(&self) -> Result<(Vec<RawFd>, Vec<RawFd>), ErrorStack> {
        let mut add_fd_count = 0usize;
        let mut del_fd_count = 0usize;
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_changed_fds(
                self.0,
                ptr::null_mut(),
                &mut add_fd_count as *mut usize,
                ptr::null_mut(),
                &mut del_fd_count as *mut usize,
            )
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        let mut add_fds: Vec<c_int> = vec![0; add_fd_count];
        let mut del_fds: Vec<c_int> = vec![0; del_fd_count];
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_changed_fds(
                self.0,
                add_fds.as_mut_ptr(),
                &mut add_fd_count as *mut usize,
                del_fds.as_mut_ptr(),
                &mut del_fd_count as *mut usize,
            )
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        Ok((
            add_fds.into_iter().map(RawFd::from).collect(),
            del_fds.into_iter().map(RawFd::from).collect(),
        ))
    }
}

pub trait AsyncOperation {
    fn track_raw_fd(&mut self, fd: RawFd);
    fn untrack_raw_fd(&mut self, fd: RawFd);
    fn poll_ready_fds(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), ErrorStack>>;
    fn run(&mut self) -> Result<(), ErrorStack>;
}

pub struct AsyncTask {
    job: *mut ffi::ASYNC_JOB,
    wait_ctx: AsyncWaitCtx,
    operation: Box<dyn AsyncOperation>,
    op_error: Result<(), ErrorStack>,
}

impl AsyncTask {
    pub fn new<T>(operation: T) -> Result<Self, ErrorStack>
    where
        T: AsyncOperation + 'static,
    {
        let wait_ctx = AsyncWaitCtx::new()?;
        Ok(AsyncTask {
            job: ptr::null_mut(),
            wait_ctx,
            operation: Box::new(operation),
            op_error: Ok(()),
        })
    }

    fn poll_run(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), ErrorStack>> {
        let mut ret: c_int = 0;

        let r = unsafe {
            ffi::ASYNC_start_job(
                &mut self.job,
                self.wait_ctx.as_ptr(),
                &mut ret,
                Some(start_job),
                self as *mut Self as *mut c_void,
                mem::size_of::<*mut Self>(),
            )
        };

        loop {
            match r {
                ffi::ASYNC_ERR => return Poll::Ready(Err(ErrorStack::get())),
                ffi::ASYNC_NO_JOBS => {
                    // no available jobs, yield now and wake later
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                ffi::ASYNC_PAUSE => {
                    let (add, del) = self.wait_ctx.get_changed_fds()?;
                    for fd in add {
                        self.operation.track_raw_fd(fd);
                    }
                    for fd in del {
                        self.operation.untrack_raw_fd(fd);
                    }
                    ready!(self.operation.poll_ready_fds(cx))?;
                }
                ffi::ASYNC_FINISH => return Poll::Ready(mem::replace(&mut self.op_error, Ok(()))),
                _ => unreachable!(),
            }
        }
    }
}

extern "C" fn start_job(arg: *mut c_void) -> c_int {
    let mut task = ptr::NonNull::new(arg as *mut AsyncTask).unwrap();
    let task = unsafe { task.as_mut() };
    task.op_error = task.operation.run();
    0
}

impl Future for AsyncTask {
    type Output = Result<(), ErrorStack>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_run(cx)
    }
}
