---- MODULE Sweep ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES n

vars == <<n>>

Fault(message) == Assert(FALSE, message)
Division(left, right) == IF (left >= 0) /\ (right >= 1) THEN left \div right ELSE Fault("a division with a negative number or a divisor below 1")
SeqItems(seq) == {seq[index] : index \in DOMAIN seq}

InRange ==
    /\ (n \in 0..1)

Init ==
    /\ n = 0

Action_inc ==
    IF InRange /\ (n < 1)
    THEN /\ n' = (n + 1)
    ELSE FALSE

End_top ==
    (n = 1)

Stop ==
    IF (~InRange) \/ End_top
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \/ Action_inc
    \/ Stop

Rule_wide ==
    InRange => ((~(LET Ok(item) == ((6 >= 0) /\ ((3 - item) >= 1)) IN LET Hit(item) == (((6 >= 0) /\ ((3 - item) >= 1)) /\ (~(Division(6, (3 - item)) > (3 + n)))) IN \A at \in 1..Len(<<1, 2, 3>>) : Ok(<<1, 2, 3>>[at]) \/ \E earlier \in 1..(at - 1) : Hit(<<1, 2, 3>>[earlier]))) \/ (\A item \in SeqItems(<<1, 2, 3>>) : (Division(6, (3 - item)) > (3 + n))))

Judged_wide ==
    InRange => (LET Ok(item) == ((6 >= 0) /\ ((3 - item) >= 1)) IN LET Hit(item) == (((6 >= 0) /\ ((3 - item) >= 1)) /\ (~(Division(6, (3 - item)) > (3 + n)))) IN \A at \in 1..Len(<<1, 2, 3>>) : Ok(<<1, 2, 3>>[at]) \/ \E earlier \in 1..(at - 1) : Hit(<<1, 2, 3>>[earlier]))

Rule_some ==
    InRange => ((~(LET Ok(item) == ((6 >= 0) /\ ((3 - item) >= 1)) IN LET Hit(item) == (((6 >= 0) /\ ((3 - item) >= 1)) /\ (Division(6, (3 - item)) >= (3 - n))) IN \A at \in 1..Len(<<1, 2, 3>>) : Ok(<<1, 2, 3>>[at]) \/ \E earlier \in 1..(at - 1) : Hit(<<1, 2, 3>>[earlier]))) \/ (\E item \in SeqItems(<<1, 2, 3>>) : (Division(6, (3 - item)) >= (3 - n))))

Judged_some ==
    InRange => (LET Ok(item) == ((6 >= 0) /\ ((3 - item) >= 1)) IN LET Hit(item) == (((6 >= 0) /\ ((3 - item) >= 1)) /\ (Division(6, (3 - item)) >= (3 - n))) IN \A at \in 1..Len(<<1, 2, 3>>) : Ok(<<1, 2, 3>>[at]) \/ \E earlier \in 1..(at - 1) : Hit(<<1, 2, 3>>[earlier]))

====
