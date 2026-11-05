#include <juce_core/juce_core.h>
#include <juce_events/juce_events.h>

#if JUCE_WASM

namespace juce
{

bool Thread::createNativeThread(Thread::Priority)
{
    // Standalone WASM ohne native JUCE-Threads.
    return false;
}

void Thread::killThread()
{
    // Kein nativer Thread vorhanden.
}

bool MessageManager::postMessageToSystemQueue(MessageBase*)
{
    // Keine native GUI-Message-Queue im Headless-Betrieb.
    return false;
}

} // namespace juce

#endif