# Hook

是防御和进攻的核心主战场.

Hook有多种:IAT hook/消息 hook等,但在攻防种90%以上都是inline hook内联钩子
## inline hook

将函数序言,替换为一个绝对跳转指令,强行跳转到edr或监控程序中.edr检查参数,如果觉得没问题就把替换掉的序言在一个trampoline的独立内存区执行一遍,然后跳回被调用的函数中继续执行.这个总结对不对?

## EDR应用的hook

启动任一进程,edr都会强行在进程中注入自己的dll.这个dll启动后,会在ntdll.dll里面几百个关键api头部钉上jmp钩子.一旦木马调用这些api,就会触发这些钩子,edr就能审视调用意图.


## Hook使用场景

1. 浏览器注入--银行木马:木马把自己的dll注入chrome.exe中,对chrome底层处理ssl加密的函数下hook.在密码被chrome加密前,hook拦截并偷走明文密码
2. 用户态rootkit:木马给系统NtQuerySystemInformation （查询进程列表）函数下hook.当任务管理器去内核要进程列表,hook会截获返回信息,把返回信息中自己的进程删掉,之后把修改后的名单交给任务管理器.
3. 键盘记录:通过调用win提供的合法机制SetWindowsHookEx(WH_KEYBOARD_LL),全局监听所有的键盘敲击事件.

360个人版杀软有edr hook吗?