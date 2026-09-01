

Routine(例程/例行程序)是从汇编中继承来的.在微软文档体系中:
1. Function（函数）：通常用来指代那些供普通开发者调用的、偏上层的 Win32 API。比如您用 C# 写个画图软件，调用 CreateWindow，微软通常叫它 Function
2. Routine（例程）：这是一个带有特权的词汇。当阅读 Windows Driver Kit (WDK)驱动开发文档，或者像 Rtl（Run-Time Library）、Nt 开头的未公开底层 API时，微软几乎清一色地使用 Routine.它暗示着：这不是一个简单的业务逻辑，这是一个直接和操作系统内核、内存管理器、中断控制器打交道的“系统级服务指令”。比如常见的 ISR（Interrupt Service Routine，中断服务例程）