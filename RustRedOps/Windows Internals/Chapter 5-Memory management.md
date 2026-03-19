# Memory management

有任何与本书冲突的,除非能使用windbg验证,否则以本书为准

## Introduction to the memory manager

By default, the virtual size of a process on 32-bit Windows is 2 GB.与大多数文献介绍的有一些区别,大多数都会说x86的进程虚拟内存是4GB,这里更加细致的解释为2GB,因为内核态会占用2GB  
A 32-bit process can grow to be up to 3 GB on 32-bit Windows and to 4 
GB on 64-bit Windows.   
The process virtual address space size on 64-bit Windows 8 and Server 2012 is 8192 
GB (8 TB) and on 64 bit Windows 8.1 (and later) and Server 2012 R2 (and later), it is 128 TB    
win8以后的版本中,虽然寄存器是64位,但硬件只实现了48位的虚拟地址寻址,即256TB,在win8内核限制只允许用户态使用8TB,在win8.1之后用户态占用一半是128TB.  
win10/11寻址从48位提到57位(总空间128PB),在某些版本中用户态空间被提升到256TB.  
如果完全实现64位的虚拟地址寻址,总空间会扩大到16EB


>对红队的意义:在红队工具开发中应当注意用户空间的虚拟内存大小是2GB,如果寻址大约2GB可能会出现错误


The maximum amount 
of physical memory currently supported by Windows ranges from 2 GB to 24 TB, depending on which 
version and edition of Windows you are running. Because the virtual address space might be larger or 
smaller than the physical memory on the machine, the memory manager has two primary tasks:
* Translating, or mapping, a process’s virtual address space into physical memory so that when a thread running in the context of that process reads or writes to the virtual address space, the correct physical address is referenced. (The subset of a process’s virtual address space that is physically resident is called the working set. Working sets are described in more detail in the section “Working sets” later in this chapter.)  
   * version指内核或发布版本代号(Windows 11 (NT 10.0.2xxxx), Windows Server2022);edition指同一个version下的Home(家庭版)/Pro(专业版),不同的edition支持不同的内存上限(Windows 11 Home：上限 128 GB/Windows 11 Pro：上限 2 TB)
   * 支持的物理内存由cpu物理寻址位数(32/64)/内核支持的成本/操作系统版本,这三者最少的决定.
   * 不同的os版本支持的最大物理内存不同,需要具体分析

* Paging分页 some of the contents of memory to disk when it becomes overcommitted超额承诺—that is, when running threads try to use more physical memory than is currently available—and bringing the contents back into physical memory when needed
