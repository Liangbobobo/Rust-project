;; desync模式:称为去同步/栈脱敏模式.当劫持或借用目标进程中一个已存在的合法工作线程(如已有的主线程,或threadpool线程)时,上游本来就是合法的系统代码.此时用desync模式在现有合法栈上做嫁接,轻量且逼真
;; synthetic合成模式,用于在目标进程中新创建一个线程,这个线程本身是从私有内存启动的,没有任何合法的调用栈.此时必须用synthetic模式从零捏造一套以RtlUserThreadStart为起点的合法假栈


;;
;; code responsible for call stack spoofing via desync(masm)
;;

;;
;; export
;;

;; proto是prototype缩写,代表函数声明
Spoof proto

;; 总结一下 一个pe分别有多少.data .text .rdata等节区
.data

